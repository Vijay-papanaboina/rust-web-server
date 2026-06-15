#!/bin/bash
set -e

SERVER_URL="http://127.0.0.1:7878"
RANDOM_NUM=$RANDOM
USERNAME="user_$RANDOM_NUM"
EMAIL="user_${RANDOM_NUM}@example.com"
PASSWORD="securepassword"

# ANSI color codes
GREEN='\033[0;32m'
RED='\033[0;31m'
NC='\033[0m' # No Color

assert_status() {
  local response="$1"
  local expected="$2"
  local test_name="$3"
  if echo "$response" | grep -q "$expected"; then
    echo -e "${GREEN}[PASS] ${test_name}${NC}"
  else
    echo -e "${RED}[FAIL] ${test_name} (Expected '$expected')${NC}"
    echo "Response received:"
    echo "$response"
    exit 1
  fi
}

echo "=================================================="
echo "          RUNNING INTEGRATION TESTS               "
echo "=================================================="

# 1. Valid Registration
echo -e "\n--- Test 1: Valid User Registration ---"
REG_RESPONSE=$(curl -s -i -X POST "$SERVER_URL/register" \
  -H "Content-Type: application/json" \
  -d "{\"username\": \"$USERNAME\", \"email\": \"$EMAIL\", \"password\": \"$PASSWORD\"}")
assert_status "$REG_RESPONSE" "HTTP/1.1 201 Created" "Valid Registration"

# 2. Duplicate Registration
echo -e "\n--- Test 2: Duplicate Registration (Same Email) ---"
DUP_RESPONSE=$(curl -s -i -X POST "$SERVER_URL/register" \
  -H "Content-Type: application/json" \
  -d "{\"username\": \"$USERNAME\", \"email\": \"$EMAIL\", \"password\": \"$PASSWORD\"}")
assert_status "$DUP_RESPONSE" "HTTP/1.1 500 Internal Server Error" "Duplicate Registration returns 500"

# 3. Invalid Login (Wrong Password)
echo -e "\n--- Test 3: Invalid Login (Wrong Password) ---"
WRONG_PWD_RESPONSE=$(curl -s -i -X POST "$SERVER_URL/login" \
  -H "Content-Type: application/json" \
  -d "{\"email\": \"$EMAIL\", \"password\": \"wrongpassword\"}")
assert_status "$WRONG_PWD_RESPONSE" "HTTP/1.1 401 Unauthorized" "Invalid Login (Wrong Password) returns 401"

# 4. Invalid Login (Non-existent Email)
echo -e "\n--- Test 4: Invalid Login (Non-existent Email) ---"
NO_USER_RESPONSE=$(curl -s -i -X POST "$SERVER_URL/login" \
  -H "Content-Type: application/json" \
  -d "{\"email\": \"nonexistent@example.com\", \"password\": \"password\"}")
assert_status "$NO_USER_RESPONSE" "HTTP/1.1 401 Unauthorized" "Invalid Login (Non-existent Email) returns 401"

# 5. Valid Login
echo -e "\n--- Test 5: Valid Login ---"
LOGIN_RESPONSE=$(curl -s -X POST "$SERVER_URL/login" \
  -H "Content-Type: application/json" \
  -d "{\"email\": \"$EMAIL\", \"password\": \"$PASSWORD\"}")

# Extract token
TOKEN=$(echo "$LOGIN_RESPONSE" | grep -o '"token":"[^"]*' | grep -o '[^"]*$')
if [ -z "$TOKEN" ]; then
  echo -e "${RED}[FAIL] Failed to extract JWT token from login response${NC}"
  exit 1
else
  echo -e "${GREEN}[PASS] Valid Login & Token Extraction${NC}"
fi

# 6. Fetch User Profile (Valid Token)
echo -e "\n--- Test 6: Fetch Profile with Valid Token ---"
PROFILE_RESPONSE=$(curl -s -i -X GET "$SERVER_URL/user" \
  -H "Authorization: Bearer $TOKEN")
assert_status "$PROFILE_RESPONSE" "HTTP/1.1 200 OK" "Fetch Profile (Valid Token) returns 200"

# 7. Fetch User Profile (Invalid Token)
echo -e "\n--- Test 7: Fetch Profile with Invalid Token ---"
BAD_TOKEN_RESPONSE=$(curl -s -i -X GET "$SERVER_URL/user" \
  -H "Authorization: Bearer invalidtokenhere")
assert_status "$BAD_TOKEN_RESPONSE" "HTTP/1.1 401 Unauthorized" "Fetch Profile (Invalid Token) returns 401"

# 8. Fetch User Profile (Missing Token)
echo -e "\n--- Test 8: Fetch Profile with Missing Token ---"
MISSING_TOKEN_RESPONSE=$(curl -s -i -X GET "$SERVER_URL/user")
assert_status "$MISSING_TOKEN_RESPONSE" "HTTP/1.1 401 Unauthorized" "Fetch Profile (Missing Token) returns 401"

echo -e "\n=================================================="
echo -e "      ${GREEN}ALL TESTS PASSED SUCCESSFULLY!${NC}      "
echo -e "=================================================="
