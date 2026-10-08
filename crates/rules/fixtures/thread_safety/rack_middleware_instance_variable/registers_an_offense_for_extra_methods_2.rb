class TestMiddleware
  def foo
    @a = 1
    ^^^^^^ Avoid instance variables in Rack middleware.
  end

  def initialize(app)
    @app = app
  end

  def call(env)
    @app.call(env)
  end
end
