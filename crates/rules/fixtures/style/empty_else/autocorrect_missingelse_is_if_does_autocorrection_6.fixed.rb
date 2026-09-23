def foo
  if @params
    case @params[:x]
    when :a
      :b
    end
  else
    :c
  end
end
