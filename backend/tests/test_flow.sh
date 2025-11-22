set -e

API="http://localhost:3000"
USER_ID="00000000-0000-0000-0000-000000000001"

echo "Starting Smoke Test..."

# Seed Test User
echo "0. Seeding Test User..."
docker compose exec -T db psql -U postgres -d fyp -c "
INSERT INTO auth.users (id, email) VALUES ('$USER_ID', 'smoke@test.com') ON CONFLICT (id) DO NOTHING;
INSERT INTO profiles (id, email, role) VALUES ('$USER_ID', 'smoke@test.com', 'admin') ON CONFLICT (id) DO NOTHING;
"

# Health Check
echo "1. Checking Health..."
curl -f "$API/health"
echo -e "\n✓ Health OK"

# Create Project
echo "2. Creating Project..."
RESPONSE=$(curl -s -X POST "$API/projects" \
  -H "Content-Type: application/json" \
  -H "x-user-id: $USER_ID" \
  -d '{
    "title": "Super Bot",
    "abstract_text": "It walks",
    "year": 2025,
    "author_name": "Smoke Tester",
    "cover_image_key": "uploads/smoke.jpg"
  }')

if [[ $RESPONSE == *"error"* ]]; then
  echo "Error creating project: $RESPONSE"
  exit 1
fi

PROJECT_ID=$(echo "$RESPONSE" | grep -o '"id":"[^"]*"' | cut -d'"' -f4)
echo "Created project: $PROJECT_ID"

# Admin Approve
echo "3. Approving Project..."
HTTP_CODE=$(curl -s -o /dev/null -w "%{http_code}" -X PUT "$API/admin/projects/$PROJECT_ID/status" \
  -H "Content-Type: application/json" \
  -H "x-user-id: $USER_ID" \
  -d 'true')

if [ "$HTTP_CODE" -eq 200 ]; then
  echo "Project Approved"
else
  echo "Failed to approve: $HTTP_CODE"
  exit 1
fi

# Verify in List
echo "4. Verifying Project in Public List..."
LIST_RESPONSE=$(curl -s "$API/projects")
if echo "$LIST_RESPONSE" | grep -q "$PROJECT_ID"; then
  echo "Project found in public list"
else
  echo "Project not found in list"
  exit 1
fi

# Vote
echo "5. Voting..."
curl -sf -X POST "$API/projects/$PROJECT_ID/vote" \
  -H "x-user-id: $USER_ID" >/dev/null
echo "Vote cast"

# Double Vote Protection
echo "6. Testing Double Vote Protection..."
HTTP_CODE=$(curl -s -o /dev/null -w "%{http_code}" -X POST "$API/projects/$PROJECT_ID/vote" \
  -H "x-user-id: $USER_ID")
if [ "$HTTP_CODE" -eq 409 ]; then
  echo "Double vote blocked (409 Conflict)"
else
  echo "Expected 409, got $HTTP_CODE"
  exit 1
fi

echo ""
echo "All Tests Passed!"
