class TestMiddleware
  def call(env)
    @app.call(env)
  end

  def initialize(app)
    @app = app
    @some_var = 2
    ^^^^^^^^^^^^^ Avoid instance variables in Rack middleware.
    @options = 1
  end
end
