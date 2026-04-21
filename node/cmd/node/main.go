package main

import (
	"context"
	"flag"
	"fmt"
	"log"
	"net"
	"net/http"
	"os"
	"proxyswarm/node/internal/engine"
	"proxyswarm/node/internal/pb"
	"proxyswarm/node/internal/service"
	"proxyswarm/node/internal/stats"
	"time"

	"github.com/improbable-eng/grpc-web/go/grpcweb"
	"google.golang.org/grpc"
	"google.golang.org/grpc/peer"
)

func main() {
	port := flag.Int("port", 9090, "The gRPC/Web port")
	flag.Parse()

	listenAddr := os.Getenv("GRPC_LISTEN")
	if listenAddr == "" {
		listenAddr = fmt.Sprintf(":%d", *port)
	}

	masterKey := os.Getenv("MASTER_KEY")
	if masterKey == "" {
		log.Fatal("MASTER_KEY environment variable is required")
	}

	lis, err := net.Listen("tcp", listenAddr)
	if err != nil {
		log.Fatalf("failed to listen: %v", err)
	}

	s := grpc.NewServer(
		grpc.UnaryInterceptor(loggingUnaryInterceptor),
		grpc.MaxRecvMsgSize(128*1024*1024),
		grpc.MaxSendMsgSize(128*1024*1024),
	)
	mgr := engine.NewManager()
	if restored, err := mgr.RestoreLastConfig(context.Background()); err != nil {
		log.Printf("failed to restore last deployed configuration: %v", err)
	} else if restored {
		log.Printf("restored last deployed configuration")
	}
	sts := stats.NewCollector()
	nodeService := &service.NodeServiceServer{
		MasterKeyHash: service.HashMasterKey(masterKey),
		Manager:       mgr,
		Stats:         sts,
	}

	pb.RegisterNodeServiceServer(s, nodeService)

	wrappedServer := grpcweb.WrapServer(s,
		grpcweb.WithOriginFunc(func(origin string) bool { return true }), // Allow all for demo
	)

	handler := func(res http.ResponseWriter, req *http.Request) {
		res.Header().Set("Access-Control-Allow-Origin", "*")
		res.Header().Set("Access-Control-Allow-Headers", "Content-Type, X-Grpc-Web, X-User-Agent")
		res.Header().Set("Access-Control-Allow-Methods", "POST, OPTIONS")

		if req.Method == http.MethodOptions {
			res.WriteHeader(http.StatusNoContent)
			return
		}

		if wrappedServer.IsGrpcWebRequest(req) {
			wrappedServer.ServeHTTP(res, req)
			return
		}
		// Fallback to other handlers if needed
		http.DefaultServeMux.ServeHTTP(res, req)
	}

	httpServer := &http.Server{
		Addr:    listenAddr,
		Handler: http.HandlerFunc(handler),
	}

	log.Printf("Starting Node on %s...", listenAddr)
	if err := httpServer.Serve(lis); err != nil {
		log.Fatalf("failed to serve: %v", err)
	}
}

func loggingUnaryInterceptor(
	ctx context.Context,
	req interface{},
	info *grpc.UnaryServerInfo,
	handler grpc.UnaryHandler,
) (interface{}, error) {
	start := time.Now()
	peerAddr := "unknown"
	if p, ok := peer.FromContext(ctx); ok && p.Addr != nil {
		peerAddr = p.Addr.String()
	}

	log.Printf("[grpc] start method=%s peer=%s req=%T", info.FullMethod, peerAddr, req)
	resp, err := handler(ctx, req)
	duration := time.Since(start)
	if err != nil {
		log.Printf("[grpc] done method=%s peer=%s duration=%s err=%v", info.FullMethod, peerAddr, duration, err)
		return resp, err
	}

	log.Printf("[grpc] done method=%s peer=%s duration=%s resp=%T", info.FullMethod, peerAddr, duration, resp)
	return resp, nil
}
