package handlers

import (
	"encoding/json"
	"testing"
	"time"

	"superpowers/user-service/models"
)

func TestToDTO(t *testing.T) {
	extends := `{"dept":"销售部","pos":"经理"}`
	user := &models.User{
		ID:        1,
		Username:  "john",
		Email:     "john@example.com",
		Phone:     "13800138000",
		Nickname:  "John",
		Status:    1,
		Extends:   json.RawMessage(extends),
		Roles:     []models.Role{{Name: "admin"}},
		CreatedAt: time.Now(),
		UpdatedAt: time.Now(),
	}

	dto, err := ToDTO(user)
	if err != nil {
		t.Fatalf("ToDTO failed: %v", err)
	}

	if dto.Username != "john" {
		t.Errorf("expected username john, got %s", dto.Username)
	}

	if len(dto.extends) != 2 {
		t.Errorf("expected 2 extends, got %d", len(dto.extends))
	}
}

func TestToDTO_EmptyExtends(t *testing.T) {
	user := &models.User{
		ID:        1,
		Username:  "john",
		Email:     "john@example.com",
		Status:    1,
		Extends:   nil,
		Roles:     []models.Role{},
		CreatedAt: time.Now(),
		UpdatedAt: time.Now(),
	}

	dto, err := ToDTO(user)
	if err != nil {
		t.Fatalf("ToDTO failed: %v", err)
	}

	if dto.Username != "john" {
		t.Errorf("expected username john, got %s", dto.Username)
	}

	if dto.extends != nil && len(dto.extends) != 0 {
		t.Errorf("expected empty extends, got %v", dto.extends)
	}
}

func TestToDTO_InvalidExtendsJSON(t *testing.T) {
	user := &models.User{
		ID:        1,
		Username:  "john",
		Extends:   json.RawMessage(`{"invalid"`),
		Roles:     []models.Role{},
		CreatedAt: time.Now(),
		UpdatedAt: time.Now(),
	}

	_, err := ToDTO(user)
	if err == nil {
		t.Error("expected error for invalid JSON, got nil")
	}
}

func TestUserDTO_MarshalJSON(t *testing.T) {
	dto := &UserDTO{
		ID:       1,
		Username: "john",
		Email:    "john@example.com",
		extends: map[string]interface{}{
			"dept": "销售部",
			"pos":  "经理",
		},
	}

	data, err := json.Marshal(dto)
	if err != nil {
		t.Fatalf("MarshalJSON failed: %v", err)
	}

	var result map[string]interface{}
	json.Unmarshal(data, &result)

	if result["username"] != "john" {
		t.Errorf("expected username john, got %v", result["username"])
	}

	if result["dept"] != "销售部" {
		t.Errorf("expected dept 销售部, got %v", result["dept"])
	}

	if result["pos"] != "经理" {
		t.Errorf("expected pos 经理, got %v", result["pos"])
	}
}

func TestUserDTO_MarshalJSON_EmptyExtends(t *testing.T) {
	dto := &UserDTO{
		ID:       1,
		Username: "john",
		extends:  nil,
	}

	data, err := json.Marshal(dto)
	if err != nil {
		t.Fatalf("MarshalJSON failed: %v", err)
	}

	var result map[string]interface{}
	json.Unmarshal(data, &result)

	if result["username"] != "john" {
		t.Errorf("expected username john, got %v", result["username"])
	}
}

func TestUserDTO_MarshalJSON_BaseFieldPriority(t *testing.T) {
	dto := &UserDTO{
		ID:       1,
		Username: "john",
		extends: map[string]interface{}{
			"username": "override",
			"id":       "999",
			"dept":     "销售部",
		},
	}

	data, err := json.Marshal(dto)
	if err != nil {
		t.Fatalf("MarshalJSON failed: %v", err)
	}

	var result map[string]interface{}
	json.Unmarshal(data, &result)

	if result["username"] != "john" {
		t.Errorf("expected username john (base field priority), got %v", result["username"])
	}

	if result["dept"] != "销售部" {
		t.Errorf("expected dept 销售部, got %v", result["dept"])
	}
}

func TestParseExtends(t *testing.T) {
	req := map[string]interface{}{
		"username": "john",
		"email":    "john@example.com",
		"dept":     "销售部",
		"pos":      "经理",
	}

	extends := ParseExtends(req)

	if len(extends) != 2 {
		t.Errorf("expected 2 extends, got %d", len(extends))
	}

	if extends["dept"] != "销售部" {
		t.Errorf("expected dept 销售部, got %v", extends["dept"])
	}
}

func TestParseExtends_AllBaseFields(t *testing.T) {
	req := map[string]interface{}{
		"id":         1,
		"username":   "john",
		"email":      "test@example.com",
		"phone":      "123",
		"nickname":   "nick",
		"status":     1,
		"roles":      []string{"admin"},
		"profile":    map[string]interface{}{},
		"created_at": "2024-01-01",
		"updated_at": "2024-01-02",
	}

	extends := ParseExtends(req)

	if len(extends) != 0 {
		t.Errorf("expected 0 extends (all base fields), got %d", len(extends))
	}
}

func TestParseExtends_NumberValues(t *testing.T) {
	req := map[string]interface{}{
		"username": "john",
		"age":      float64(30),
		"salary":   12000.50,
	}

	extends := ParseExtends(req)

	if len(extends) != 2 {
		t.Errorf("expected 2 extends, got %d", len(extends))
	}

	if extends["age"] != float64(30) {
		t.Errorf("expected age 30, got %v (type: %T)", extends["age"], extends["age"])
	}

	if extends["salary"] != 12000.50 {
		t.Errorf("expected salary 12000.50, got %v", extends["salary"])
	}
}
