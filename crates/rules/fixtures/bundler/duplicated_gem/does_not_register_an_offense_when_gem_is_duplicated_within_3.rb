case
when Dir.exist?(local)
  gem 'rubocop', path: local
when ENV['RUBOCOP_VERSION'] == 'master'
  # no-op, do nothing club
else
  gem 'rubocop', '~> 0.90.0'
end
