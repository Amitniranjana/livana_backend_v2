INSERT INTO users (id, first_name, last_name, email, password, phone_no)
VALUES ('149e5792-2f6a-4aba-9124-ed3b8df3f27a', 'Test', 'Provider', 'provider_xyz123@example.com', 'abc', '+911234567891') ON CONFLICT DO NOTHING;

INSERT INTO carecrew_providers (id, user_id, name, service_type, city, is_active)
VALUES ('149e5792-2f6a-4aba-9124-ed3b8df3f27a', '149e5792-2f6a-4aba-9124-ed3b8df3f27a', 'Test Provider', 'plumber', 'Hyderabad', true) ON CONFLICT DO NOTHING;

INSERT INTO services (id, provider_id, service_name, category, price, description, experience, location)
VALUES ('a343fc81-f35c-494f-a90f-67d40514a418', '149e5792-2f6a-4aba-9124-ed3b8df3f27a', 'expert plumber', 'plumber', 210, 'bcvjgykghkghkhkgkl,ghoul,,', '2', 'Hyderabad') ON CONFLICT DO NOTHING;
