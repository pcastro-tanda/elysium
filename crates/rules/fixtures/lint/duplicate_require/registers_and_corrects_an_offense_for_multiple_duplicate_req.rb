require 'foo'
require_relative '../bar'
require 'foo/baz'
require_relative '../bar'
^^^^^^^^^^^^^^^^^^^^^^^^^ Duplicate `require_relative` detected.
Kernel.require 'foo'
^^^^^^^^^^^^^^^^^^^^ Duplicate `require` detected.
require 'quux'
