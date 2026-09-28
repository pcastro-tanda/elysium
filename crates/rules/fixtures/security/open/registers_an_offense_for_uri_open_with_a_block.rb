::URI.open("| #{foo}") do |f|
      ^^^^ The use of `::URI.open` is a serious security risk.
  f.gets
end
