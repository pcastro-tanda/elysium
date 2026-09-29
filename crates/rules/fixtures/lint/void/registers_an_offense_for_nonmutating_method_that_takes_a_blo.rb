[1,2,3].collect do |n|
^^^^^^^^^^^^^^^^^^^^^^ Method `#collect` used in void context. Did you mean `#each`?
  n.to_s
end
"done"
