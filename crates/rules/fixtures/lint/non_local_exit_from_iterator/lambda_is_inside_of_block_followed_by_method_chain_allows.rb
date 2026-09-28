RSpec.configure do |config|
  # some configuration

  if Gem.loaded_specs["paper_trail"].version < Gem::Version.new("4.0.0")
    current_behavior = ActiveSupport::Deprecation.behavior
    ActiveSupport::Deprecation.behavior = lambda do |message, callstack|
      return if message =~ /foobar/
      Array.wrap(current_behavior).each do |behavior|
        behavior.call(message, callstack)
      end
    end

    # more configuration
  end
end
