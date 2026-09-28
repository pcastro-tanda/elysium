group(*LIVE_ENVS) do
  gem 'admin_ui'
end

group(*LIVE_ENVS) do
^^^^^^^^^^^^^^^^^ Gem group `*LIVE_ENVS` already defined on line 1 of the Gemfile.
  gem 'public_ui'
end
