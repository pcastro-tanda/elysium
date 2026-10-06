class TestMiddleware
  def initialize(app)
    @app = app
  end

  def call(env)
    @app.call(env)
    @a = 1
    ^^^^^^ Avoid instance variables in Rack middleware.
  end

  def foo
    @a = 1
    ^^^^^^ Avoid instance variables in Rack middleware.
  end
end
