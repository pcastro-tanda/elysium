class TestMiddleware
  def initialize(app)
    @app = app
    @counter = 0
    ^^^^^^^^^^^^ Avoid instance variables in Rack middleware.
  end

  def call(env)
    @app.call(env)
  ensure
    @counter += 1
    ^^^^^^^^ Avoid instance variables in Rack middleware.
  end
end
