# encoding: utf-8
^^^^^^^^^^^^^^^^^ Sorbet/EnforceSigilOrder: Magic comments should be in the following order: encoding, typed, warn_indent, frozen_string_literal.
# foo
# frozen_string_literal: true
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Sorbet/EnforceSigilOrder: Magic comments should be in the following order: encoding, typed, warn_indent, frozen_string_literal.
# bar: true
# warn_indent: true
^^^^^^^^^^^^^^^^^^^ Sorbet/EnforceSigilOrder: Magic comments should be in the following order: encoding, typed, warn_indent, frozen_string_literal.
# baz: "Hello"
# typed: true
^^^^^^^^^^^^^ Sorbet/EnforceSigilOrder: Magic comments should be in the following order: encoding, typed, warn_indent, frozen_string_literal.
# coding: utf-8
^^^^^^^^^^^^^^^ Sorbet/EnforceSigilOrder: Magic comments should be in the following order: encoding, typed, warn_indent, frozen_string_literal.
# another foo
class Foo; end
