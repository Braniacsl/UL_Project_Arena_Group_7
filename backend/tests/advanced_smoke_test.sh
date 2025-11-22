set -e

API="http://localhost:3000"
USER_ID="00000000-0000-0000-0000-000000000001"

echo "Starting Advanced Smoke Test..."

# Seed Admin User
echo "0. Seeding Admin User..."
docker compose exec -T db psql -U postgres -d fyp -c "
INSERT INTO auth.users (id, email) VALUES ('$USER_ID', 'admin@test.com') ON CONFLICT (id) DO NOTHING;
INSERT INTO profiles (id, email, role) VALUES ('$USER_ID', 'admin@test.com', 'admin') ON CONFLICT (id) DO NOTHING;
"

# Create Test Projects
echo "1. Creating Test Projects..."

ID_AI=$(curl -s -X POST "$API/projects" \
  -H "Content-Type: application/json" \
  -H "x-user-id: $USER_ID" \
  -d '{
    "title": "Generative AI Spec",
    "abstract_text": "LLM Stuff",
    "year": 2025,
    "author_name": "AI Student",
    "cover_image_key": "uploads/ai.png"
  }' | grep -o '"id":"[^"]*"' | cut -d'"' -f4)

ID_WEB=$(curl -s -X POST "$API/projects" \
  -H "Content-Type: application/json" \
  -H "x-user-id: $USER_ID" \
  -d '{
    "title": "React Dashboard",
    "abstract_text": "Web Stuff",
    "year": 2024,
    "author_name": "Web Student",
    "cover_image_key": "uploads/web.png"
  }' | grep -o '"id":"[^"]*"' | cut -d'"' -f4)

echo "✓ Created: AI Project (2025) & Web Project (2024)"

# Approve Projects
echo "2. Approving Projects..."
CODE_A=$(curl -s -o /dev/null -w "%{http_code}" -X PUT "$API/admin/projects/$ID_AI/status" \
  -H "Content-Type: application/json" \
  -H "x-user-id: $USER_ID" \
  -d 'true')
CODE_B=$(curl -s -o /dev/null -w "%{http_code}" -X PUT "$API/admin/projects/$ID_WEB/status" \
  -H "Content-Type: application/json" \
  -H "x-user-id: $USER_ID" \
  -d 'true')

if [ "$CODE_A" -ne 200 ] || [ "$CODE_B" -ne 200 ]; then
  echo "✗ Failed to approve. Status codes: $CODE_A, $CODE_B"
  exit 1
fi
echo "✓ Projects approved"

# Test Year Filter
echo "3. Testing Year Filter (2025)..."
YEAR_RESPONSE=$(curl -s "$API/projects?year=2025")
if echo "$YEAR_RESPONSE" | grep -q "$ID_AI"; then
  echo "✓ Year filter working"
else
  echo "✗ Year filter failed"
  exit 1
fi

# Test Search Filter
echo "4. Testing Search Filter ('Web')..."
SEARCH_RESPONSE=$(curl -s "$API/projects?search=Web")
if echo "$SEARCH_RESPONSE" | grep -q "$ID_WEB"; then
  echo "Search filter working"
else
  echo "Search filter failed"
  exit 1
fi

# Test Empty Results
echo "5. Testing Empty Results ('Banana')..."
BANANA_RESPONSE=$(curl -s "$API/projects?search=Banana")
if [ "$(echo "$BANANA_RESPONSE" | wc -c)" -le 5 ]; then
  echo "Empty results handled correctly"
else
  echo "Found unexpected results"
  exit 1
fi

# Test 404
echo "6. Testing 404 on Invalid ID..."
RANDOM_ID="00000000-0000-0000-0000-000000000000"
HTTP_CODE=$(curl -s -o /dev/null -w "%{http_code}" -X PUT "$API/admin/projects/$RANDOM_ID/status" \
  -H "Content-Type: application/json" \
  -H "x-user-id: $USER_ID" \
  -d 'true')

if [ "$HTTP_CODE" -eq 404 ]; then
  echo "404 returned correctly"
else
  echo "Expected 404, got $HTTP_CODE"
  exit 1
fi

echo ""
echo "All Tests Passed"
