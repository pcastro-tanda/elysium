yielding_method do
  define_method(:bad_method) do |arg0, arg1 = 1, *args, foo:, bar: nil, **kwargs, &block|
    if arg0
      arg0 + arg1
    end
  end
end
