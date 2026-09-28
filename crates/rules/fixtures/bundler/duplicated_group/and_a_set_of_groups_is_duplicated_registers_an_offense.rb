group :test, :development do
  gem 'rubocop'
end
group :development, :test do
^^^^^^^^^^^^^^^^^^^^^^^^^ Gem group `:development, :test` already defined on line 1 of the Gemfile.
  gem 'rubocop-rails'
end
