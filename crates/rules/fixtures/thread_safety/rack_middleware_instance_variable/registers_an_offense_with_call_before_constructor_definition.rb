class TestMiddleware
  def call(env)
    @app.call(env)
  end

  def initialize(app)
    @app = app
    @foo = 1
    ^^^^^^^^ Avoid instance variables in Rack middleware.
  end
end
