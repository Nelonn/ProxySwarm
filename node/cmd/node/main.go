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
	"proxyswarm/node/internal/logging"
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

	masterKey := os.Getenv("PS_MASTER_KEY")
	if masterKey == "" {
		log.Fatal("PS_MASTER_KEY environment variable is required")
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
		logging.Warnf("failed to restore last deployed configuration: %v", err)
	} else if restored {
		logging.Infof("restored last deployed configuration")
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

		if isNodeManageRequest(req, wrappedServer) && req.Method == http.MethodOptions {
			res.WriteHeader(http.StatusNoContent)
			return
		}

		if wrappedServer.IsGrpcWebRequest(req) {
			wrappedServer.ServeHTTP(res, req)
			return
		}
		if isNodeManagePath(req.URL.Path) {
			http.Error(res, "gRPC-Web request required", http.StatusBadRequest)
			return
		}
		// Fallback to other handlers if needed
		http.DefaultServeMux.ServeHTTP(res, req)
	}

	httpServer := &http.Server{
		Addr:    listenAddr,
		Handler: http.HandlerFunc(handler),
	}

	logging.Infof("starting node on %s", listenAddr)
	if err := httpServer.Serve(lis); err != nil {
		log.Fatalf("failed to serve: %v", err)
	}
}

func isNodeManageRequest(req *http.Request, wrappedServer *grpcweb.WrappedGrpcServer) bool {
	return wrappedServer.IsGrpcWebRequest(req) ||
		wrappedServer.IsAcceptableGrpcCorsRequest(req) ||
		isNodeManagePath(req.URL.Path)
}

func isNodeManagePath(path string) bool {
	return len(path) >= len("/proxyswarm.NodeService/") && path[:len("/proxyswarm.NodeService/")] == "/proxyswarm.NodeService/"
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

	logging.Debugf("[grpc] start method=%s peer=%s req=%T", info.FullMethod, peerAddr, req)
	resp, err := handler(ctx, req)
	duration := time.Since(start)
	if err != nil {
		logging.Warnf("[grpc] done method=%s peer=%s duration=%s err=%v", info.FullMethod, peerAddr, duration, err)
		return resp, err
	}

	logging.Debugf("[grpc] done method=%s peer=%s duration=%s resp=%T", info.FullMethod, peerAddr, duration, resp)
	return resp, nil
}
