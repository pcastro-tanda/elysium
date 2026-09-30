def targets_for(path)
  fullpath = fullpath_for(path)
  [
    Dir.glob(fullpath),
  ].flatten
    .uniq
    ^^^^^ Align `.uniq` with `.flatten` on line 5.
    .delete_if { |entry| dot_directory?(entry) }
end
