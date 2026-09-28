git 'https://github.com/rails/rails.git' do
  group :default do
    gem 'activesupport'
  end
end

git 'https://github.com/rails/rails.git' do
  group :default do
  ^^^^^^^^^^^^^^ Gem group `:default` already defined on line 2 of the Gemfile.
    gem 'actionpack'
  end
end
