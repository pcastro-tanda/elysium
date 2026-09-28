open("#{foo}.txt") do |f|
^^^^ The use of `Kernel#open` is a serious security risk.
  f.gets
end
