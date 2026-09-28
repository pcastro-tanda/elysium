gem 'rubocop'
group :development do
  gem 'rubocop', path: '/path/to/gem'
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Gem `rubocop` requirements already given on line 1 of the Gemfile.
end
