platforms :ruby do
  group :default do
    gem 'ruby-debug'
  end
end

platforms :ruby do
  group :default do
  ^^^^^^^^^^^^^^ Gem group `:default` already defined on line 2 of the Gemfile.
    gem 'sqlite3'
  end
end
