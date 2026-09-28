group :development do
  gem 'rubocop'
end
group :development, :test do
  gem 'rspec'
end
group :ci, :development do
  gem 'flog'
end
