class TestMiddleware
  def initialize(app)
    @app = app
    @foo = 1
    ^^^^^^^^ Avoid instance variables in Rack middleware.
  end

  def call(env)
    @app.call(env)
    p @foo
      ^^^^ Avoid instance variables in Rack middleware.
  end
end
