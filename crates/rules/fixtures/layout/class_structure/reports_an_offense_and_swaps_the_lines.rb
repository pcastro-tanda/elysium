class GridTask
  DESC = 'Grid Task' # grid task name OID, subclasses should set this
  extend Helpers::MakeFromFile
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^ `module_inclusion` is supposed to appear before `constants`.
end
