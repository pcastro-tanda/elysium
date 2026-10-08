MyClass = Class.new(Foreman::Renderer).send(:new)
MyClass = Class.new(Foreman::Renderer).public_send(:new)
MyClass = Class.new(StandardError).any_method
MyClass = Class.new(StandardError).tap { }
MyClass = Class.new(StandardError).send(:new).another_method
