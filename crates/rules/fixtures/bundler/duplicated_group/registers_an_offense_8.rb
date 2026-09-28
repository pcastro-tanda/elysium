path 'components' do
  group :default do
    gem 'admin_ui'
  end
end

path 'components' do
  group :default do
  ^^^^^^^^^^^^^^ Gem group `:default` already defined on line 2 of the Gemfile.
    gem 'public_ui'
  end
end
