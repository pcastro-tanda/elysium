block = proc { |_, *args| klass.new(*args) }
block.ruby2_keywords if block.respond_to?(:ruby2_keywords)
