if Dir.exist?(local)
  gem 'rubocop', path: local
elsif ENV['RUBOCOP_VERSION'] == 'master'
  gem 'rubocop', git: 'https://github.com/rubocop/rubocop.git'
elsif (version = ENV['RUBOCOP_VERSION'])
  gem 'rubocop', version
else
  gem 'rubocop', '~> 0.90.0'
end
