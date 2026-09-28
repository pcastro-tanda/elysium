gem 'a'

group :development do
  gem 'b'
  gem 'c'
  gem 'b'
  ^^^^^^^ Gems should be sorted in an alphabetical order within their section of the Gemfile. Gem `b` should appear before `c`.
end
