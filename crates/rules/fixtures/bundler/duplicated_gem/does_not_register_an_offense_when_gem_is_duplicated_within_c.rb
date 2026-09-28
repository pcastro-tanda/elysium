case
when Dir.exist?(local)
  gem 'rubocop', path: local
when ENV['RUBOCOP_VERSION'] == 'master'
  gem 'rubocop', git: 'https://github.com/rubocop/rubocop.git'
else
  gem 'rubocop', '~> 0.90.0'
end
