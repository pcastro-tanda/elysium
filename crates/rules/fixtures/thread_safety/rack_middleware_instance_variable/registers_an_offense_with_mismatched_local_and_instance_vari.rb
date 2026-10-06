class TestMiddleware
  def initialize(app)
    @foo = fsa
    ^^^^^^^^^^ Avoid instance variables in Rack middleware.
    @a = app
  end

  def call(env)
    @a.call(env)
  end
end
