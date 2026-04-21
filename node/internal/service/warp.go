package service

import (
	"bytes"
	"context"
	"encoding/base64"
	"encoding/json"
	"fmt"
	"proxyswarm/node/internal/pb"
	"io"
	"net/http"
	"os"
	"strings"
	"time"
)

func (s *NodeServiceServer) RegisterWarp(ctx context.Context, req *pb.WarpRegisterRequest) (*pb.WarpRegisterResponse, error) {
	if req.MasterKey != s.MasterKeyHash {
		return &pb.WarpRegisterResponse{Success: false, Error: "Unauthorized"}, nil
	}
	if strings.TrimSpace(req.PublicKey) == "" {
		return &pb.WarpRegisterResponse{Success: false, Error: "public_key is required"}, nil
	}

	tos := time.Now().UTC().Format("2006-01-02T15:04:05.000Z")
	hostName, _ := os.Hostname()
	requestBody, _ := json.Marshal(map[string]any{
		"key":   req.PublicKey,
		"tos":   tos,
		"type":  "PC",
		"model": "ProxySwarm",
		"name":  hostName,
	})

	httpClient := &http.Client{Timeout: 30 * time.Second}
	registerReq, _ := http.NewRequestWithContext(ctx, http.MethodPost, "https://api.cloudflareclient.com/v0a2158/reg", bytes.NewReader(requestBody))
	registerReq.Header.Set("Content-Type", "application/json")
	registerReq.Header.Set("CF-Client-Version", "a-7.21-0721")

	resp, err := httpClient.Do(registerReq)
	if err != nil {
		return &pb.WarpRegisterResponse{Success: false, Error: fmt.Sprintf("Cloudflare registration request failed: %v", err)}, nil
	}
	defer resp.Body.Close()

	if resp.StatusCode < 200 || resp.StatusCode >= 300 {
		body, _ := io.ReadAll(resp.Body)
		return &pb.WarpRegisterResponse{Success: false, Error: fmt.Sprintf("Cloudflare registration failed: %d %s", resp.StatusCode, strings.TrimSpace(string(body)))}, nil
	}

	var registerData map[string]any
	if err := json.NewDecoder(resp.Body).Decode(&registerData); err != nil {
		return &pb.WarpRegisterResponse{Success: false, Error: fmt.Sprintf("failed to decode registration response: %v", err)}, nil
	}

	id, _ := registerData["id"].(string)
	token, _ := registerData["token"].(string)
	account, _ := registerData["account"].(map[string]any)
	license, _ := account["license"].(string)
	if id == "" || token == "" {
		return &pb.WarpRegisterResponse{Success: false, Error: "Cloudflare registration returned incomplete payload"}, nil
	}

	configReq, _ := http.NewRequestWithContext(ctx, http.MethodGet, fmt.Sprintf("https://api.cloudflareclient.com/v0a2158/reg/%s", id), nil)
	configReq.Header.Set("Authorization", fmt.Sprintf("Bearer %s", token))
	configResp, err := httpClient.Do(configReq)
	if err != nil {
		return &pb.WarpRegisterResponse{Success: false, Error: fmt.Sprintf("Cloudflare config request failed: %v", err)}, nil
	}
	defer configResp.Body.Close()

	if configResp.StatusCode < 200 || configResp.StatusCode >= 300 {
		body, _ := io.ReadAll(configResp.Body)
		return &pb.WarpRegisterResponse{Success: false, Error: fmt.Sprintf("Cloudflare config fetch failed: %d %s", configResp.StatusCode, strings.TrimSpace(string(body)))}, nil
	}

	var configData map[string]any
	if err := json.NewDecoder(configResp.Body).Decode(&configData); err != nil {
		return &pb.WarpRegisterResponse{Success: false, Error: fmt.Sprintf("failed to decode config response: %v", err)}, nil
	}

	addresses := make([]string, 0, 2)
	configRoot, _ := configData["config"].(map[string]any)
	iface, _ := configRoot["interface"].(map[string]any)
	addrMap, _ := iface["addresses"].(map[string]any)
	if v4, ok := addrMap["v4"].(string); ok && v4 != "" {
		addresses = append(addresses, fmt.Sprintf("%s/32", v4))
	}
	if v6, ok := addrMap["v6"].(string); ok && v6 != "" {
		addresses = append(addresses, fmt.Sprintf("%s/128", v6))
	}

	clientID, _ := configRoot["client_id"].(string)
	reserved, err := base64.StdEncoding.DecodeString(clientID)
	if err != nil {
		reserved = []byte{}
	}

	endpoint := "engage.cloudflareclient.com:2408"
	peerPublicKey := ""
	if peers, ok := configRoot["peers"].([]any); ok && len(peers) > 0 {
		if peer, ok := peers[0].(map[string]any); ok {
			if value, ok := peer["public_key"].(string); ok {
				peerPublicKey = value
			}
			if endpointMap, ok := peer["endpoint"].(map[string]any); ok {
				if host, ok := endpointMap["host"].(string); ok && host != "" {
					endpoint = host
				}
			}
		}
	}

	return &pb.WarpRegisterResponse{
		Success: true,
		Registration: &pb.WarpRegistration{
			Id:        id,
			Token:     token,
			License:   license,
			Reserved:  reserved,
			Addresses: addresses,
			Endpoint:  endpoint,
			PeerPublicKey: peerPublicKey,
		},
	}, nil
}

func (s *NodeServiceServer) UpdateWarpLicense(ctx context.Context, req *pb.WarpLicenseUpdateRequest) (*pb.WarpLicenseUpdateResponse, error) {
	if req.MasterKey != s.MasterKeyHash {
		return &pb.WarpLicenseUpdateResponse{Success: false, Error: "Unauthorized"}, nil
	}
	if strings.TrimSpace(req.DeviceId) == "" {
		return &pb.WarpLicenseUpdateResponse{Success: false, Error: "device_id is required"}, nil
	}
	if strings.TrimSpace(req.AccessToken) == "" {
		return &pb.WarpLicenseUpdateResponse{Success: false, Error: "access_token is required"}, nil
	}
	if strings.TrimSpace(req.License) == "" {
		return &pb.WarpLicenseUpdateResponse{Success: false, Error: "license is required"}, nil
	}

	requestBody, _ := json.Marshal(map[string]any{
		"license": req.License,
	})

	url := fmt.Sprintf("https://api.cloudflareclient.com/v0a2158/reg/%s/account", req.DeviceId)
	licenseReq, _ := http.NewRequestWithContext(ctx, http.MethodPut, url, bytes.NewReader(requestBody))
	licenseReq.Header.Set("Authorization", fmt.Sprintf("Bearer %s", req.AccessToken))
	licenseReq.Header.Set("Content-Type", "application/json")

	httpClient := &http.Client{Timeout: 30 * time.Second}
	resp, err := httpClient.Do(licenseReq)
	if err != nil {
		return &pb.WarpLicenseUpdateResponse{Success: false, Error: fmt.Sprintf("Cloudflare license update request failed: %v", err)}, nil
	}
	defer resp.Body.Close()

	if resp.StatusCode < 200 || resp.StatusCode >= 300 {
		body, _ := io.ReadAll(resp.Body)
		return &pb.WarpLicenseUpdateResponse{
			Success: false,
			Error:   fmt.Sprintf("Cloudflare license update failed: %d %s", resp.StatusCode, strings.TrimSpace(string(body))),
		}, nil
	}

	return &pb.WarpLicenseUpdateResponse{
		Success: true,
		License: req.License,
	}, nil
}
