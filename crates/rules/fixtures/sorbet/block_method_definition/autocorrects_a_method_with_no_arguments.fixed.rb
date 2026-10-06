yielding_method do
  define_method(:bad_method) do
    if arg0
      arg0 + arg1
    end
  end
end
