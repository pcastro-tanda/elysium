class TestMiddleware
  def initialize(app)
    @app = app
    foo = SomeClass.new
    instance_variable_set(:counter, 1)
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Avoid instance variables in Rack middleware.
  end

  def call(env)
    @app.call(env)
    instance_variable_get("@counter")
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Avoid instance variables in Rack middleware.
  end
end
