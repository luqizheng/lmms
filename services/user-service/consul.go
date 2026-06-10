package main

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io/ioutil"
	"log"
	"net"
	"net/http"

	"github.com/google/uuid"
	"superpowers/user-service/config"
)

type AgentServiceCheck struct {
	HTTP                          string `json:"http"`
	Interval                      string `json:"interval"`
	Timeout                       string `json:"timeout"`
	DeregisterCriticalServiceAfter string `json:"deregister_critical_service_after"`
}

type AgentServiceRegistration struct {
	ID      string              `json:"id"`
	Name    string              `json:"name"`
	Address string              `json:"address"`
	Port    int                 `json:"port"`
	Check   AgentServiceCheck   `json:"check"`
}

type ConsulRegistrar struct {
	cfg       *config.ConsulConfig
	serviceID string
}

func NewConsulRegistrar(cfg *config.ConsulConfig) *ConsulRegistrar {
	serviceID := loadOrGenerateServiceID(cfg.ServiceIDFile)
	return &ConsulRegistrar{
		cfg:       cfg,
		serviceID: serviceID,
	}
}

func loadOrGenerateServiceID(filePath string) string {
	data, err := ioutil.ReadFile(filePath)
	if err == nil {
		content := string(bytes.TrimSpace(data))
		if content != "" {
			log.Printf("Loaded existing service_id from %s", filePath)
			return content
		}
	}

	newID := fmt.Sprintf("%s-%s", "user-service", uuid.New().String())
	if err := ioutil.WriteFile(filePath, []byte(newID), 0644); err != nil {
		log.Printf("Warning: failed to write service_id to %s: %v", filePath, err)
	} else {
		log.Printf("Generated new service_id: %s", newID)
	}

	return newID
}

func getOutboundIP() string {
	if addrs, err := net.InterfaceAddrs(); err == nil {
		for _, addr := range addrs {
			if ipnet, ok := addr.(*net.IPNet); ok && !ipnet.IP.IsLoopback() && !ipnet.IP.IsLinkLocalUnicast() {
				if ip4 := ipnet.IP.To4(); ip4 != nil {
					return ip4.String()
				}
			}
		}
	}

	if conn, err := net.Dial("udp", "8.8.8.8:80"); err == nil {
		defer conn.Close()
		if ip := conn.LocalAddr().(*net.UDPAddr).IP; ip != nil {
			if ip4 := ip.To4(); ip4 != nil && !ip4.IsLoopback() {
				return ip4.String()
			}
			if !ip.IsLoopback() && !ip.IsUnspecified() {
				return ip.String()
			}
		}
	}

	log.Printf("Warning: no routable IP detected, fallback to 127.0.0.1 (跨主机/跨容器健康检查将失败)")
	return "127.0.0.1"
}

func (r *ConsulRegistrar) Register(port int) error {
	host := getOutboundIP()
	registration := AgentServiceRegistration{
		ID:      r.serviceID,
		Name:    r.cfg.ServiceName,
		Address: host,
		Port:    port,
		Check: AgentServiceCheck{
			HTTP:                          fmt.Sprintf("http://%s:%d/health", host, port),
			Interval:                      "10s",
			Timeout:                       "5s",
			DeregisterCriticalServiceAfter: "1m",
		},
	}

	jsonData, err := json.Marshal(registration)
	if err != nil {
		return fmt.Errorf("failed to marshal registration: %w", err)
	}

	url := fmt.Sprintf("%s/v1/agent/service/register", r.cfg.Addr)
	log.Printf("Consul register -> POST %s", url)
	log.Printf("Consul register payload: %s", string(jsonData))
	resp, err := http.Post(url, "application/json", bytes.NewBuffer(jsonData))
	if err != nil {
		return fmt.Errorf("failed to register service: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		body, _ := ioutil.ReadAll(resp.Body)
		return fmt.Errorf("failed to register service: HTTP %d, body: %s", resp.StatusCode, string(body))
	}

	log.Printf("Successfully registered service %s with id %s at %s:%d", r.cfg.ServiceName, r.serviceID, host, port)
	return nil
}

func (r *ConsulRegistrar) Deregister() error {
	url := fmt.Sprintf("%s/v1/agent/service/deregister/%s", r.cfg.Addr, r.serviceID)
	resp, err := http.Post(url, "", nil)
	if err != nil {
		return fmt.Errorf("failed to deregister service: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		body, _ := ioutil.ReadAll(resp.Body)
		return fmt.Errorf("failed to deregister service: HTTP %d, body: %s", resp.StatusCode, string(body))
	}

	log.Printf("Successfully deregistered service %s", r.serviceID)
	return nil
}

func (r *ConsulRegistrar) ServiceID() string {
	return r.serviceID
}