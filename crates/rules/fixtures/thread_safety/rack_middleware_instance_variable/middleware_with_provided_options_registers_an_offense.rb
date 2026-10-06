class TestMiddleware
  def initialize(app, options)
    @app = app
    @options = options
    ^^^^^^^^^^^^^^^^^^ Avoid instance variables in Rack middleware.
  end

  def call(env)
    if options[:noop]
      [200, {}, []]
    else
      @app.call(env)
    end
  end
end
