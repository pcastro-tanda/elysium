with_options options: false do |merger|
  merger.invoke
  with_another_method do |another_receiver|
    merger.invoke(another_receiver)
  end
end
