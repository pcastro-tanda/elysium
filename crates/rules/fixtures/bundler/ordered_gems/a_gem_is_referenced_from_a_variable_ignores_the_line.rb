gem 'rspec'
gem ENV['env_key_undefined'] if ENV.key?('env_key_undefined')
gem 'rubocop'
