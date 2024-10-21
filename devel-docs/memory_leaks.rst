Apparent memory leaks
=====================

If you run Valgrind or another memory checker on a program that uses librsvg, or
``rsvg-convert``, you may get false positives.  This chapter explains why these occur by
giving some examples of false positives from Valgrind.

Note that there may be real memory leaks, and they should be fixed!  This chapter just
explains why not everything that is reported as a memory leak is in fact a leak.

Example: false leak in fontconfig's code for ``FcPattern``
----------------------------------------------------------

.. code-block::

   $ valgrind --leak-check=full --track-origins=yes rsvg-convert -o foo.png foo.svg

   ==5712== 5,378 (512 direct, 4,866 indirect) bytes in 1 blocks are definitely lost in loss record 1,463 of 1,487
   ==5712==    at 0x484D82F: realloc (in /usr/libexec/valgrind/vgpreload_memcheck-amd64-linux.so)
   ==5712==    by 0x5577D88: FcPatternObjectInsertElt (fcpat.c:516)
   ==5712==    by 0x557BE08: FcPatternObjectAddWithBinding (fcpat.c:711)
   ==5712==    by 0x557315F: UnknownInlinedFun (fcpat.c:738)
   ==5712==    by 0x557315F: UnknownInlinedFun (fcpat.c:884)
   ==5712==    by 0x557315F: FcDefaultSubstitute (fcdefault.c:257)
   ==5712==    by 0x544E15D: UnknownInlinedFun (pangofc-fontmap.c:2066)
   ==5712==    by 0x544E15D: UnknownInlinedFun (pangofc-fontmap.c:2143)
   ==5712==    by 0x544E15D: pango_fc_font_map_load_fontset (pangofc-fontmap.c:2245)
   ==5712==    by 0x4D74FBC: UnknownInlinedFun (itemize.c:892)
   ==5712==    by 0x4D74FBC: UnknownInlinedFun (itemize.c:952)
   ==5712==    by 0x4D74FBC: pango_itemize_with_font (itemize.c:1564)
   ==5712==    by 0x4D880D1: pango_layout_check_lines.part.0.lto_priv.0 (pango-layout.c:4894)
   ==5712==    by 0x4D7D58D: UnknownInlinedFun (pango-layout.c:4786)
   ==5712==    by 0x4D7D58D: pango_layout_get_extents_internal.lto_priv.0 (pango-layout.c:2925)
   ==5712==    by 0x4D7D735: pango_layout_get_size (pango-layout.c:3166)
   ==5712==    by 0x6987FF: pango::auto::layout::Layout::size (layout.rs:321)
   ==5712==    by 0x542342: librsvg::text::MeasuredSpan::from_span (text.rs:357)
   ==5712==    by 0x33465C: librsvg::text::MeasuredChunk::from_chunk::{{closure}} (text.rs:146)

Fontconfig is a library to enumerate the system's fonts based on different configuration
parameters.  It gets used by Pango, GNOME's library for text rendering, and in turn by
librsvg.

Is the report above a leak in fontconfig?  No.  Let's look at
``FcPatternObjectInsertElt()``, the function that called ``realloc()`` in the stack
trace above.  `From fccpat.c
<https://gitlab.freedesktop.org/fontconfig/fontconfig/-/blob/fd0753af/src/fcpat.c#L498-552>`_:

.. code-block:: c

   int s = p->size + 16;
   if (p->size)
   {
       FcPatternElt *e0 = FcPatternElts(p);
       e = (FcPatternElt *) realloc (e0, s * sizeof (FcPatternElt));
       if (!e) /* maybe it was mmapped */
       {
           e = malloc(s * sizeof (FcPatternElt));
           if (e)
       	memcpy(e, e0, FcPatternObjectCount (p) * sizeof (FcPatternElt));
       }
   }
   else
       e = (FcPatternElt *) malloc (s * sizeof (FcPatternElt));
   if (!e)
       return FcFalse;
   p->elts_offset = FcPtrToOffset (p, e);

The code inside the first ``if (p->size)`` essentially does a ``realloc()`` to resize an
existing array, or ``malloc()`` to allocate a new one.  However, **that pointer is encoded
instead of stored plainly** in the last line: ``p->elts_offset = FcPtrToOffset (p, e)``.
If you look at the definition of `FcPtrToOffset
<https://gitlab.freedesktop.org/fontconfig/fontconfig/-/blob/fd0753af/src/fcint.h#L161>`_,
you will see that it encodes the pointer as the offset between a base location and the
pointer's location itself (``p`` and ``e`` respectively in the code above).

What Valgrind thinks is, "oh, they just allocated this memory and immediately obliterated
the pointer to it, so it must be a leak!".  However, what actually happens is that
fontconfig doesn't store that pointer plainly, but mangles it somehow.  Then it un-mangles
it with `FcPatternElts
<https://gitlab.freedesktop.org/fontconfig/fontconfig/-/blob/fd0753af/src/fcint.h#L232>`_
to access it, and frees the memory properly `later in FcPatternDestroy()
<https://gitlab.freedesktop.org/fontconfig/fontconfig/-/blob/fd0753af/src/fcpat.c#L439-443>`_.


Example: false leak in Rust's hashbrown crate
---------------------------------------------

.. code-block::

   $ valgrind --leak-check=full --track-origins=yes rsvg-convert -o foo.png foo.svg

   ==5712== 708 bytes in 3 blocks are possibly lost in loss record 1,380 of 1,487
   ==5712==    at 0x48487B4: malloc (in /usr/libexec/valgrind/vgpreload_memcheck-amd64-linux.so)
   ==5712==    by 0x79E17B: alloc::alloc::alloc (alloc.rs:87)
   ==5712==    by 0x79E316: alloc::alloc::Global::alloc_impl (alloc.rs:169)
   ==5712==    by 0x79E439: <alloc::alloc::Global as core::alloc::Allocator>::allocate (alloc.rs:229)
   ==5712==    by 0x784A57: hashbrown::raw::alloc::inner::do_alloc (alloc.rs:11)
   ==5712==    by 0x7C8ECD: hashbrown::raw::RawTableInner<A>::new_uninitialized (mod.rs:1086)
   ==5712==    by 0x7C926B: hashbrown::raw::RawTableInner<A>::fallible_with_capacity (mod.rs:1115)
   ==5712==    by 0x7CA2C5: hashbrown::raw::RawTableInner<A>::prepare_resize (mod.rs:1359)
   ==5712==    by 0x7C7424: hashbrown::raw::RawTable<T,A>::reserve_rehash (mod.rs:1432)
   ==5712==    by 0x7C7068: hashbrown::raw::RawTable<T,A>::reserve (mod.rs:652)
   ==5712==    by 0x7C81F5: hashbrown::raw::RawTable<T,A>::insert (mod.rs:731)
   ==5712==    by 0x77828B: hashbrown::map::HashMap<K,V,S,A>::insert (map.rs:1508)

Hashbrown is a Rust crate that implements an efficient hash table.  Let's look at the code from `hashbrown::raw::RawTableInner<A>::new_uninitialized <https://github.com/rust-lang/hashbrown/blob/1d2c1a81d1b53285decbd64410a21a90112613d7/src/raw/mod.rs#L1080-L1085>`_:

.. code-block:: rust

   let ptr: NonNull<u8> = match do_alloc(&alloc, layout) {
       Ok(block) => block.cast(),
       Err(_) => return Err(fallibility.alloc_err(layout)),
   };

   let ctrl = NonNull::new_unchecked(ptr.as_ptr().add(ctrl_offset));

First it calls ``do_alloc`` which is essentially ``malloc()`` underneath.  Then, **it adds
an offset to the resulting ptr**, where it does ``ptr.as_ptr().add(ctrl_offset)``.  You
can see a description of the actual layout `in the declaration of the ctrl field
<https://github.com/rust-lang/hashbrown/blob/1d2c1a81d1b53285decbd64410a21a90112613d7/src/raw/mod.rs#L374-L376>`_.
Similar to the example above for fontconfig, Valgrind sees that the code immediately
obliterates the only existing pointer to the newly-allocated memory, and thus thinks that
it leaks the corresponding memory.


Example: false leak in Rust's regex crate
-----------------------------------------

.. code-block::

   $ valgrind --leak-check=full --track-origins=yes rsvg-convert -o foo.png foo.svg

   ==5712== 42 bytes in 6 blocks are possibly lost in loss record 793 of 1,487
   ==5712==    at 0x48487B4: malloc (in /usr/libexec/valgrind/vgpreload_memcheck-amd64-linux.so)
   ==5712==    by 0xA99074: alloc (alloc.rs:87)
   ==5712==    by 0xA99074: alloc_impl (alloc.rs:169)
   ==5712==    by 0xA99074: allocate (alloc.rs:229)
   ==5712==    by 0xA99074: allocate_in<u8, alloc::alloc::Global> (raw_vec.rs:185)
   ==5712==    by 0xA99074: with_capacity_in<u8, alloc::alloc::Global> (raw_vec.rs:132)
   ==5712==    by 0xA99074: with_capacity_in<u8, alloc::alloc::Global> (mod.rs:609)
   ==5712==    by 0xA99074: to_vec<u8, alloc::alloc::Global> (slice.rs:227)
   ==5712==    by 0xA99074: to_vec<u8, alloc::alloc::Global> (slice.rs:176)
   ==5712==    by 0xA99074: to_vec_in<u8, alloc::alloc::Global> (slice.rs:501)
   ==5712==    by 0xA99074: clone<u8, alloc::alloc::Global> (mod.rs:2483)
   ==5712==    by 0xA99074: <alloc::string::String as core::clone::Clone>::clone (string.rs:1861)
   ==5712==    by 0x761A56: <T as alloc::borrow::ToOwned>::to_owned (borrow.rs:90)
   ==5712==    by 0x765986: <alloc::string::String as alloc::string::ToString>::to_string (string.rs:2486)
   ==5712==    by 0x7692DB: regex::compile::Compiler::c (compile.rs:373)
   ==5712==    by 0x76C748: regex::compile::Compiler::c_concat (compile.rs:532)
   ==5712==    by 0x769124: regex::compile::Compiler::c (compile.rs:384)
   ==5712==    by 0x76927B: regex::compile::Compiler::c (compile.rs:364)
   ==5712==    by 0x76E4B1: regex::compile::Compiler::c_repeat_zero_or_one (compile.rs:614)
   ==5712==    by 0x76E302: regex::compile::Compiler::c_repeat (compile.rs:592)
   ==5712==    by 0x769031: regex::compile::Compiler::c (compile.rs:388)
   ==5712==    by 0x76C748: regex::compile::Compiler::c_concat (compile.rs:532)

This is related to the example above for the hashbrown crate.  The regex crate, for regular expressions, builds a hash table with the names of captures.  It `allocates a string for the name of each capture and inserts it in a hash table <https://github.com/rust-lang/regex/blob/9ca3099/src/compile.rs#L371-L378>`_:

.. code-block:: rust

   hir::GroupKind::CaptureName { index, ref name } => {
       if index as usize >= self.compiled.captures.len() {
           let n = name.to_string();
           self.compiled.captures.push(Some(n.clone()));
           self.capture_name_idx.insert(n, index as usize);
       }
       self.c_capture(2 * index as usize, &g.hir)
   }

The allocation happens in ``name.to_string()``.  Two lines below, the string gets inserted
into the ``self.capture_name_idx`` hash table.

By looking at the `declaration for the capture_name_idx field
<https://github.com/rust-lang/regex/blob/9ca3099/src/compile.rs#L35>`_, we see that it is
a ``HashMap<String, usize>``.  However, that ``HashMap`` is in fact a hashbrown table, as
in the previous section.  Since hashbrown uses a special encoding for its internal
pointers, Valgrind thinks that the original pointer to the string is lost.
