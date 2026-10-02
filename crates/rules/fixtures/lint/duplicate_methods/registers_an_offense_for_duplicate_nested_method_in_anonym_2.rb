Module.new do
  def foo
    def some_method
      implement 1
    end
  end

  def foo
  ^^^^^^^ Method `Object#foo` is defined at both example.rb:2 and example.rb:8.
    def some_method
    ^^^^^^^^^^^^^^^ Method `Object#some_method` is defined at both example.rb:3 and example.rb:9.
      implement 2
    end
  end
end
