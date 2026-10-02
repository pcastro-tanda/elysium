Gem::Specification.new do |spec|
  spec.name = 'your_cool_gem_name'
  spec.date = Time.now.strftime('%Y-%m-%d')
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Do not set `date` in gemspec.
  spec.bindir = 'exe'
end
