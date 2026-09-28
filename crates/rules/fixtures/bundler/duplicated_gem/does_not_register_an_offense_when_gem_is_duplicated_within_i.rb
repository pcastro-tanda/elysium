if Dir.exist?(local)
  gem 'rubocop', path: local
  gem 'flog', path: local
else
  gem 'rubocop', '~> 0.90.0'
end
