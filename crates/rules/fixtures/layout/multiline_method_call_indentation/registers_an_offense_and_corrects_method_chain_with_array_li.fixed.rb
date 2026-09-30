def targets_for(path)
  fullpath = fullpath_for(path)
  [
    Dir.glob(fullpath),
  ].flatten
   .uniq
   .delete_if { |entry|dot_directory?(entry) }
end
