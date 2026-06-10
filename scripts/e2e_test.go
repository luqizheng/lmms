package main

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"os"
	"time"
)

const (
	AuthServiceURL   = "http://localhost:8082"
	TokenServerURL   = "http://localhost:8081"
	API GatewayURL    = "http://localhost:8080"
	UserServiceURL   = "http://localhost:8083"
)

type TestResult struct {
	Name    string
	Passed  bool
	Message string
}

func main() {
	results := []TestResult{}

	fmt.Println("Starting E2E Tests...")
	fmt.Println("========================================")

	results = append(results, testHealthEndpoints()...)
	results = append(results, testAuthFlow()...)

	printResults(results)

	passed := countPassed(results)
	total := len(results)

	if passed == total {
		fmt.Printf("\nAll tests passed! (%d/%d)\n", passed, total)
		os.Exit(0)
	} else {
		fmt.Printf("\nSome tests failed! (%d/%d passed)\n", passed, total)
		os.Exit(1)
	}
}

func testHealthEndpoints() []TestResult {
	results := []TestResult{}

	services := map[string]string{
		"API Gateway":   API GatewayURL + "/health",
		"Token Server":  TokenServerURL + "/health",
		"Auth Service":  AuthServiceURL + "/health",
		"User Service":  UserServiceURL + "/health",
	}

	for name, url := range services {
		resp, err := http.Get(url)
		if err != nil {
			results = append(results, TestResult{
				Name:    name + " Health Check",
				Passed:  false,
				Message: err.Error(),
			})
			continue
		}
		defer resp.Body.Close()

		if resp.StatusCode == http.StatusOK {
			results = append(results, TestResult{
				Name:    name + " Health Check",
				Passed:  true,
				Message: "OK",
			})
		} else {
			results = append(results, TestResult{
				Name:    name + " Health Check",
				Passed:  false,
				Message: fmt.Sprintf("Status: %d", resp.StatusCode),
			})
		}
	}

	return results
}

func testAuthFlow() []TestResult {
	results := []TestResult{}

	fmt.Println("\n--- Testing Authentication Flow ---")

	username := fmt.Sprintf("testuser_%d", time.Now().Unix())
	password := "testpass123"

	registerReq := map[string]string{
		"username": username,
		"password": password,
		"email":    username + "@example.com",
	}
	registerBody, _ := json.Marshal(registerReq)

	resp, err := http.Post(
		AuthServiceURL+"/api/auth/register",
		"application/json",
		bytes.NewBuffer(registerBody),
	)
	if err != nil {
		results = append(results, TestResult{
			Name:    "User Registration",
			Passed:  false,
			Message: err.Error(),
		})
		return results
	}
	defer resp.Body.Close()

	body, _ := io.ReadAll(resp.Body)
	if resp.StatusCode == http.StatusCreated {
		results = append(results, TestResult{
			Name:    "User Registration",
			Passed:  true,
			Message: "User created successfully",
		})
	} else {
		results = append(results, TestResult{
			Name:    "User Registration",
			Passed:  false,
			Message: fmt.Sprintf("Status: %d, Body: %s", resp.StatusCode, string(body)),
		})
		return results
	}

	loginReq := map[string]string{
		"username": username,
		"password": password,
	}
	loginBody, _ := json.Marshal(loginReq)

	resp, err = http.Post(
		AuthServiceURL+"/api/auth/login",
		"application/json",
		bytes.NewBuffer(loginBody),
	)
	if err != nil {
		results = append(results, TestResult{
			Name:    "User Login",
			Passed:  false,
			Message: err.Error(),
		})
		return results
	}
	defer resp.Body.Close()

	loginRespBody, _ := io.ReadAll(resp.Body)
	if resp.StatusCode == http.StatusOK {
		var loginResp map[string]interface{}
		json.Unmarshal(loginRespBody, &loginResp)

		if code, ok := loginResp["code"].(string); ok && code != "" {
			results = append(results, TestResult{
				Name:    "User Login",
				Passed:  true,
				Message: "Login successful, got auth code",
			})

			results = append(results, testTokenExchange(code)...)
		} else {
			results = append(results, TestResult{
				Name:    "User Login",
				Passed:  false,
				Message: "No auth code in response",
			})
		}
	} else {
		results = append(results, TestResult{
			Name:    "User Login",
			Passed:  false,
			Message: fmt.Sprintf("Status: %d", resp.StatusCode),
		})
	}

	resp, err = http.Post(AuthServiceURL+"/api/auth/logout", "application/json", nil)
	if err == nil && resp.StatusCode == http.StatusOK {
		results = append(results, TestResult{
			Name:    "User Logout",
			Passed:  true,
			Message: "Logout successful",
		})
	} else {
		results = append(results, TestResult{
			Name:    "User Logout",
			Passed:  false,
			Message: "Logout failed",
		})
	}

	return results
}

func testTokenExchange(code string) []TestResult {
	results := []TestResult{}

	accessReq := map[string]string{
		"grant_type":      "authorization_code",
		"code":            code,
		"client_id":       "test-client",
		"client_secret":   "test-secret",
	}
	accessBody, _ := json.Marshal(accessReq)

	resp, err := http.Post(
		TokenServerURL+"/api/tokens/access",
		"application/json",
		bytes.NewBuffer(accessBody),
	)

	if err != nil {
		results = append(results, TestResult{
			Name:    "Token Exchange",
			Passed:  false,
			Message: err.Error(),
		})
		return results
	}
	defer resp.Body.Close()

	body, _ := io.ReadAll(resp.Body)
	if resp.StatusCode == http.StatusOK {
		var tokenResp map[string]interface{}
		json.Unmarshal(body, &tokenResp)

		if accessToken, ok := tokenResp["access_token"].(string); ok && accessToken != "" {
			results = append(results, TestResult{
				Name:    "Token Exchange",
				Passed:  true,
				Message: "Got access token",
			})

			results = append(results, testJWKS()...)
			results = append(results, testProtectedResource(accessToken)...)
		} else {
			results = append(results, TestResult{
				Name:    "Token Exchange",
				Passed:  false,
				Message: "No access token in response",
			})
		}
	} else {
		results = append(results, TestResult{
			Name:    "Token Exchange",
			Passed:  false,
			Message: fmt.Sprintf("Status: %d, Body: %s", resp.StatusCode, string(body)),
		})
	}

	return results
}

func testJWKS() []TestResult {
	results := []TestResult{}

	resp, err := http.Get(TokenServerURL + "/api/tokens/.well-known/jwks.json")
	if err != nil {
		results = append(results, TestResult{
			Name:    "JWKS Endpoint",
			Passed:  false,
			Message: err.Error(),
		})
		return results
	}
	defer resp.Body.Close()

	if resp.StatusCode == http.StatusOK {
		results = append(results, TestResult{
			Name:    "JWKS Endpoint",
			Passed:  true,
			Message: "JWKS retrieved successfully",
		})
	} else {
		results = append(results, TestResult{
			Name:    "JWKS Endpoint",
			Passed:  false,
			Message: fmt.Sprintf("Status: %d", resp.StatusCode),
		})
	}

	return results
}

func testProtectedResource(accessToken string) []TestResult {
	results := []TestResult{}

	req, _ := http.NewRequest("GET", UserServiceURL+"/api/users/me", nil)
	req.Header.Set("Authorization", "Bearer "+accessToken)

	client := &http.Client{Timeout: 10 * time.Second}
	resp, err := client.Do(req)

	if err != nil {
		results = append(results, TestResult{
			Name:    "Protected Resource Access",
			Passed:  false,
			Message: err.Error(),
		})
		return results
	}
	defer resp.Body.Close()

	if resp.StatusCode == http.StatusOK || resp.StatusCode == http.StatusNotFound {
		results = append(results, TestResult{
			Name:    "Protected Resource Access",
			Passed:  true,
			Message: fmt.Sprintf("Resource access attempted, status: %d", resp.StatusCode),
		})
	} else {
		results = append(results, TestResult{
			Name:    "Protected Resource Access",
			Passed:  false,
			Message: fmt.Sprintf("Status: %d", resp.StatusCode),
		})
	}

	return results
}

func printResults(results []TestResult) {
	fmt.Println("\n========================================")
	fmt.Println("Test Results:")
	fmt.Println("========================================")

	for _, r := range results {
		status := "PASS"
		color := "\033[92m"
		if !r.Passed {
			status = "FAIL"
			color = "\033[91m"
		}
		fmt.Printf("%s[%s]\033[0m %s: %s\n", color, status, r.Name, r.Message)
	}
}

func countPassed(results []TestResult) int {
	count := 0
	for _, r := range results {
		if r.Passed {
			count++
		}
	}
	return count
}
