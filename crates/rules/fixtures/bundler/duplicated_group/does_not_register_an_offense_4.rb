git 'https://github.com/rubocop/rubocop.git' do
  group :default do
    gem 'rubocop'
  end
end

git 'https://github.com/rails/rails.git' do
  group :default do
    gem 'activesupport'
    gem 'actionpack'
  end
end
