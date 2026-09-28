source 'https://rubygems.pkg.github.com/private-org' do
  group :development do
    gem 'rubocop'
  end
end

source 'https://rubygems.pkg.github.com/private-org' do
  group :development do
  ^^^^^^^^^^^^^^^^^^ Gem group `:development` already defined on line 2 of the Gemfile.
    gem 'rubocop-rails'
  end
end

group :development do
  gem 'rubocop-performance'
end
