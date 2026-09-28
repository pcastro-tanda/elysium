Gem::Specification.new do |spec|
  %w(rubocop-performance rubocop-rails).each { |dep| spec.add_dependency dep }
  spec.add_dependency 'parser'
end
