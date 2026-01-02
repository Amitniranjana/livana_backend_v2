# 1. AWS Provider Setup
provider "aws" {
  region = "us-east-1"
}

# 2. API Gateway
resource "aws_apigatewayv2_api" "livana_api" {
  name          = "LivanaBackendAPI"
  protocol_type = "HTTP"
}

# 3. Integration
resource "aws_apigatewayv2_integration" "livana_integration" {
  api_id           = aws_apigatewayv2_api.livana_api.id
  integration_type = "HTTP_PROXY"
  integration_uri  = "http://52.55.209.189:9090"
  integration_method = "ANY"
}

# 4. Route
resource "aws_apigatewayv2_route" "livana_route" {
  api_id    = aws_apigatewayv2_api.livana_api.id
  route_key = "$default"
  target    = "integrations/${aws_apigatewayv2_integration.livana_integration.id}"
}

# 5. Stage
resource "aws_apigatewayv2_stage" "livana_stage" {
  api_id      = aws_apigatewayv2_api.livana_api.id
  name        = "$default"
  auto_deploy = true
}

# 6. Output 
output "api_endpoint" {
  description = "Backend API ka URL"
  value       = aws_apigatewayv2_stage.livana_stage.invoke_url
}