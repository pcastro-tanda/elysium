class TestMiddleware
  def initialize(app)
    @app = app
    @counter = Concurrent::AtomicReference.new(0)
    @unsafe_counter = 0
    ^^^^^^^^^^^^^^^^^^^ Avoid instance variables in Rack middleware.
  end

  def call(env)
    @app.call(env)
  ensure
    @unsafe_counter += 1
    ^^^^^^^^^^^^^^^ Avoid instance variables in Rack middleware.
    @counter.update { |ref| ref + 1 }
  end
end
