group :test, foo: true do
  gem 'activesupport'
end

group :test, foo: false do
  gem 'rspec'
end
