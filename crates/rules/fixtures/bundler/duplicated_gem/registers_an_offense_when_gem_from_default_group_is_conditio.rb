gem 'rubocop'
if Dir.exist? local
  gem 'rubocop', path: local
  ^^^^^^^^^^^^^^^^^^^^^^^^^^ Gem `rubocop` requirements already given on line 1 of the Gemfile.
else
  gem 'rubocop', '~> 0.90.0'
  ^^^^^^^^^^^^^^^^^^^^^^^^^^ Gem `rubocop` requirements already given on line 1 of the Gemfile.
end
