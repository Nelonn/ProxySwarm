package service

import (
	"context"
	"crypto/sha256"
	"proxyswarm/node/internal/engine"
	"proxyswarm/node/internal/pb"
	"proxyswarm/node/internal/stats"

	"encoding/hex"
	"google.golang.org/grpc/codes"
	"google.golang.org/grpc/status"
)

type NodeServiceServer struct {
	pb.UnimplementedNodeServiceServer
	MasterKeyHash string
	Manager       *engine.Manager
	Stats         *stats.Collector
}

func (s *NodeServiceServer) UpdateConfig(ctx context.Context, req *pb.FullConfig) (*pb.UpdateResponse, error) {
	if req.MasterKey != s.MasterKeyHash {
		return nil, status.Error(codes.Unauthenticated, "unauthorized")
	}

	err := s.Manager.Update(ctx, req)
	if err != nil {
		return nil, status.Error(codes.Internal, err.Error())
	}

	return &pb.UpdateResponse{Success: true}, nil
}

func (s *NodeServiceServer) GetStatus(ctx context.Context, req *pb.StatusRequest) (*pb.NodeStatus, error) {
	if req.MasterKey != s.MasterKeyHash {
		return nil, status.Error(codes.Unauthenticated, "unauthorized")
	}

	hw := s.Stats.GetHardwareStats()
	inbounds := s.Manager.GetInboundStatus(ctx)
	accounts := s.Manager.GetAccountStatus(ctx)
	outbounds := s.Manager.GetOutboundStatus(ctx)

	return &pb.NodeStatus{
		Hardware:             hw,
		Inbounds:             inbounds,
		Accounts:             accounts,
		Outbounds:            outbounds,
		TotalInboundTraffic:  s.Manager.GetTotalInboundTraffic(ctx),
		TotalOutboundTraffic: s.Manager.GetTotalOutboundTraffic(ctx),
		Connections:          s.Manager.GetConnectionStats(ctx),
		SampleWindowSeconds:  s.Manager.GetSampleWindowSeconds(),
		HourlyMetrics:        s.Manager.GetHourlyMetrics(ctx, req.GetHourlyMetricsHours(), req.GetHourlyMetricsFromUnix(), req.GetHourlyMetricsToUnix()),
	}, nil
}

func (s *NodeServiceServer) IssueAcmeCertificate(ctx context.Context, req *pb.AcmeIssueRequest) (*pb.AcmeIssueResponse, error) {
	if req.MasterKey != s.MasterKeyHash {
		return &pb.AcmeIssueResponse{
			Success: false,
			Error:   "Unauthorized",
			Logs:    []string{"Authorization failed"},
		}, nil
	}

	result, err := s.Manager.IssueAcmeCertificate(ctx, engine.AcmeIssueParams{
		Email:           req.Email,
		Domain:          req.Domain,
		ChallengeType:   req.ChallengeType,
		CA:              req.Ca,
		Port:            req.Port,
		CertificatePath: req.CertificatePath,
		KeyPath:         req.KeyPath,
	})
	if err != nil {
		return &pb.AcmeIssueResponse{
			Success:    false,
			Error:      err.Error(),
			Logs:       result.Logs,
			ExpiryTime: result.ExpiryTime.Unix(),
		}, nil
	}

	return &pb.AcmeIssueResponse{
		Success:    true,
		Logs:       result.Logs,
		ExpiryTime: result.ExpiryTime.Unix(),
	}, nil
}

func HashMasterKey(masterKey string) string {
	sum := sha256.Sum256([]byte(masterKey))
	return hex.EncodeToString(sum[:])
}
