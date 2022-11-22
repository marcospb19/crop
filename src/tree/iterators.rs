use std::sync::Arc;

use super::tree_slice::SliceSpan;
use super::{Inode, Leaf, Metric, Node, Tree, TreeSlice};

/// An iterator over the leaves of trees or tree slices.
///
/// This iterator is created via the `leaves` method on
/// [`Tree`](super::Tree::leaves) and [`TreeSlice`](super::TreeSlice::leaves).
pub struct Leaves<'a, const FANOUT: usize, L: Leaf> {
    /// TODO: docs
    start: Option<&'a L::Slice>,

    /// TODO: docs
    start_been_yielded: bool,

    /// TODO: docs
    root_nodes: &'a [Arc<Node<FANOUT, L>>],

    /// TODO: docs
    end: Option<&'a L::Slice>,

    /// TODO: docs
    end_been_yielded: bool,

    /// TODO: docs
    forward_root_idx: isize,

    /// TODO: docs
    forward_path: Vec<(&'a Inode<FANOUT, L>, usize)>,

    /// TODO: docs
    forward_leaves: &'a [Arc<Node<FANOUT, L>>],

    /// TODO: docs
    forward_leaf_idx: usize,

    /// TODO: docs
    _backward_root_idx: usize,

    /// TODO: docs
    backward_path: Vec<(&'a Inode<FANOUT, L>, usize)>,

    /// TODO: docs
    _backward_leaves: &'a [Arc<Node<FANOUT, L>>],

    /// TODO: docs
    _backward_leaf_idx: usize,
}

impl<'a, const FANOUT: usize, L: Leaf> Clone for Leaves<'a, FANOUT, L> {
    #[inline]
    fn clone(&self) -> Self {
        Self {
            forward_path: self.forward_path.clone(),
            backward_path: self.backward_path.clone(),
            ..*self
        }
    }
}

impl<'a, const FANOUT: usize, L: Leaf> From<&'a Tree<FANOUT, L>>
    for Leaves<'a, FANOUT, L>
{
    #[inline]
    fn from(tree: &'a Tree<FANOUT, L>) -> Leaves<'a, FANOUT, L> {
        let (start, start_been_yielded, root_nodes) = match &*tree.root {
            Node::Leaf(leaf) => (Some(leaf.value().borrow()), false, &[][..]),
            Node::Internal(inode) => (None, true, inode.children()),
        };

        Leaves {
            start,
            start_been_yielded,
            root_nodes,
            end: None,
            end_been_yielded: true,
            forward_root_idx: -1,
            forward_path: Vec::new(),
            forward_leaves: &[],
            forward_leaf_idx: 0,
            _backward_root_idx: root_nodes.len(),
            backward_path: Vec::new(),
            _backward_leaves: &[],
            _backward_leaf_idx: 0,
        }
    }
}

impl<'a, const FANOUT: usize, L: Leaf> From<&'a TreeSlice<'a, FANOUT, L>>
    for Leaves<'a, FANOUT, L>
{
    #[inline]
    fn from(
        tree_slice: &'a TreeSlice<'a, FANOUT, L>,
    ) -> Leaves<'a, FANOUT, L> {
        let (start, start_been_yielded, root_nodes, end, end_been_yielded) =
            match &tree_slice.span {
                SliceSpan::Empty => (None, true, &[][..], None, true),

                SliceSpan::Single(slice) => {
                    (Some(*slice), false, &[][..], None, true)
                },

                SliceSpan::Multi { start, internals, end } => {
                    (Some(start.0), false, &**internals, Some(end.0), false)
                },
            };

        Leaves {
            start,
            start_been_yielded,
            root_nodes,
            end,
            end_been_yielded,
            forward_path: Vec::new(),
            forward_root_idx: -1,
            forward_leaves: &[],
            forward_leaf_idx: 0,
            _backward_root_idx: root_nodes.len(),
            backward_path: Vec::new(),
            _backward_leaves: &[],
            _backward_leaf_idx: 0,
        }
    }
}

impl<'a, const FANOUT: usize, L: Leaf> Iterator for Leaves<'a, FANOUT, L> {
    type Item = &'a L::Slice;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if !self.start_been_yielded {
            self.start_been_yielded = true;
            if self.start.is_some() {
                return self.start;
            }
        }

        if self.forward_leaf_idx == self.forward_leaves.len() {
            match next_leaves_forward(
                self.root_nodes,
                &mut self.forward_root_idx,
                &mut self.forward_path,
            ) {
                Some(leaves) => {
                    self.forward_leaves = leaves;
                    self.forward_leaf_idx = 0;
                },
                None => {
                    if !self.end_been_yielded {
                        self.end_been_yielded = true;
                        return self.end;
                    } else {
                        return None;
                    }
                },
            }
        }

        debug_assert!(
            self.forward_leaves[self.forward_leaf_idx].is_leaf(),
            "TODO: explain why"
        );

        // Safety: TODO.
        let leaf = unsafe {
            self.forward_leaves[self.forward_leaf_idx].as_leaf_unchecked()
        }
        .value()
        .borrow();

        self.forward_leaf_idx += 1;

        Some(leaf)
    }
}

fn next_leaves_forward<'a, const N: usize, L: Leaf>(
    root_nodes: &'a [Arc<Node<N, L>>],
    root_idx: &mut isize,
    path: &mut Vec<(&'a Inode<N, L>, usize)>,
) -> Option<&'a [Arc<Node<N, L>>]> {
    let mut inode = loop {
        match path.last_mut() {
            Some(&mut (inode, ref mut visited)) => {
                if inode.children().len() == *visited + 1 {
                    path.pop();
                } else {
                    *visited += 1;

                    debug_assert!(
                        inode.children()[*visited].is_internal(),
                        "TODO: explain why"
                    );

                    // Safety: TODO.
                    break unsafe {
                        inode.children()[*visited].as_internal_unchecked()
                    };
                }
            },

            None => {
                if root_nodes.len() == (*root_idx + 1) as usize {
                    return None;
                } else {
                    *root_idx += 1;
                    match &*root_nodes[*root_idx as usize] {
                        Node::Internal(inode) => {
                            break inode;
                        },
                        Node::Leaf(_) => {
                            return Some(
                                &root_nodes[*root_idx as usize
                                    ..*root_idx as usize + 1],
                            );
                        },
                    }
                }
            },
        }
    };

    loop {
        match &*inode.children()[0] {
            Node::Internal(i) => {
                path.push((inode, 0));
                inode = i;
            },
            Node::Leaf(_) => {
                return Some(inode.children());
            },
        }
    }
}

impl<'a, const FANOUT: usize, L: Leaf> DoubleEndedIterator
    for Leaves<'a, FANOUT, L>
{
    fn next_back(&mut self) -> Option<Self::Item> {
        if !self.end_been_yielded {
            self.end_been_yielded = true;
            if self.end.is_some() {
                return self.end;
            }
        }
        todo!()
    }
}

impl<'a, const FANOUT: usize, L: Leaf> std::iter::FusedIterator
    for Leaves<'a, FANOUT, L>
{
}

/// An iterator over consecutive units of a particular metric.
pub struct Units<'a, const FANOUT: usize, L: Leaf, M: Metric<L>> {
    /// TODO: docs
    start: Option<(&'a L::Slice, L::Summary)>,

    /// TODO: docs
    root_nodes: &'a [Arc<Node<FANOUT, L>>],

    /// TODO: docs
    end: Option<(&'a L::Slice, L::Summary)>,

    /// TODO: docs
    root_forward_idx: isize,

    /// # Invariant
    /// - the index in the last item of the vector is a leaf.
    forward_path: Vec<(&'a Inode<FANOUT, L>, usize)>,

    /// TODO: docs
    yielded_forward: M,

    /// TODO: docs
    _yielded_backward: M,

    /// TODO: docs
    total: M,

    /// TODO: docs
    metric: std::marker::PhantomData<M>,
}

impl<'a, const FANOUT: usize, L: Leaf, M: Metric<L>> Clone
    for Units<'a, FANOUT, L, M>
{
    #[inline]
    fn clone(&self) -> Self {
        Self {
            start: self.start.clone(),
            end: self.end.clone(),
            forward_path: self.forward_path.clone(),
            ..*self
        }
    }
}

impl<'a, const FANOUT: usize, L: Leaf, M: Metric<L>> From<&'a Tree<FANOUT, L>>
    for Units<'a, FANOUT, L, M>
{
    #[inline]
    fn from(tree: &'a Tree<FANOUT, L>) -> Units<'a, FANOUT, L, M> {
        let (start, root_nodes) = match &*tree.root {
            Node::Leaf(leaf) => (
                Some((leaf.value().borrow(), leaf.summary().clone())),
                &[][..],
            ),

            Node::Internal(inode) => (None, inode.children()),
        };

        Units {
            start,
            root_nodes,
            root_forward_idx: -1,
            forward_path: Vec::new(),
            end: None,
            yielded_forward: M::zero(),
            _yielded_backward: M::zero(),
            total: M::measure(tree.summary()),
            metric: std::marker::PhantomData,
        }
    }
}

impl<'a, const FANOUT: usize, L: Leaf, M: Metric<L>>
    From<&'a TreeSlice<'a, FANOUT, L>> for Units<'a, FANOUT, L, M>
{
    #[inline]
    fn from(
        tree_slice: &'a TreeSlice<'a, FANOUT, L>,
    ) -> Units<'a, FANOUT, L, M> {
        let (start, root_nodes, end) = match &tree_slice.span {
            SliceSpan::Single(slice) => {
                (Some((*slice, tree_slice.summary().clone())), &[][..], None)
            },

            SliceSpan::Multi { start, internals, end } => {
                (Some(start.clone()), &**internals, Some(end.clone()))
            },

            SliceSpan::Empty => (None, &[][..], None),
        };

        Units {
            start,
            root_nodes,
            root_forward_idx: -1,
            forward_path: Vec::new(),
            end,
            yielded_forward: M::zero(),
            _yielded_backward: M::zero(),
            total: M::measure(tree_slice.summary()),
            metric: std::marker::PhantomData,
        }
    }
}

impl<'a, const FANOUT: usize, L: Leaf, M: Metric<L>> Units<'a, FANOUT, L, M> {}

impl<'a, const FANOUT: usize, L: Leaf, M: Metric<L>> Iterator
    for Units<'a, FANOUT, L, M>
{
    type Item = TreeSlice<'a, FANOUT, L>;

    fn next(&mut self) -> Option<Self::Item> {
        let start = self.start.take();

        let (start_slice, start_summary) = match start {
            Some((slice, ref summary)) => (slice, summary),

            None => {
                match next_something_forward(
                    self.root_nodes,
                    &mut self.root_forward_idx,
                    &mut self.forward_path,
                ) {
                    Some((slice, summary)) => (slice, summary),

                    None => {
                        // Start is None and there's not a next leaf.
                        let (end_slice, end_summary) = self.end.take()?;

                        self.yielded_forward += M::one();

                        if M::measure(&end_summary) == M::zero() {
                            return Some(TreeSlice {
                                span: SliceSpan::Single(end_slice),
                                summary: end_summary,
                            });
                        } else {
                            let (end_slice, end_summary, right) =
                                M::split_left(
                                    end_slice,
                                    M::one(),
                                    &end_summary,
                                );
                            self.end = right;
                            return Some(TreeSlice {
                                span: SliceSpan::Single(end_slice),
                                summary: end_summary,
                            });
                        }
                    },
                }
            },
        };

        let start = if M::measure(start_summary) == M::zero() {
            (start_slice, start_summary.clone())
        } else {
            let (left_slice, left_summary, right) =
                M::split_left(start_slice, M::one(), start_summary);

            self.start = right;

            self.yielded_forward += M::one();

            return Some(TreeSlice {
                span: SliceSpan::Single(left_slice),
                summary: left_summary,
            });
        };

        let mut summary = start.1.clone();

        // We'll find the end if the measure of the rest of the tree is not
        // zero. To determine this we should probably check the measure at the
        // beginning and increase it by one every time we iterate.

        if self.yielded_forward == self.total {
            let mut internals = Vec::new();

            next_babagugu::<FANOUT, L, M>(
                self.root_nodes,
                &mut self.root_forward_idx,
                &mut self.forward_path,
                &mut summary,
                &mut internals,
            );

            self.start = None;

            match self.end.take() {
                Some(end) => {
                    // a) if there's self.end we need to push all the remaining
                    // nodes in the tree to internals.
                    let internals =
                        internals.drain(..).map(Arc::clone).collect();

                    summary += &end.1;

                    return Some(TreeSlice {
                        span: SliceSpan::Multi { start, internals, end },
                        summary,
                    });
                },

                None => {
                    // b) if there's no self.end we need to do the same except
                    // the last leaf in the tree is the end. except how do you
                    // know if the leaf next to the one you're on the is the
                    // last one?

                    match internals.pop() {
                        Some(last) => {
                            let mut internals = internals
                                .drain(..)
                                .map(Arc::clone)
                                .collect::<Vec<_>>();

                            let end = {
                                let mut node = &**last;

                                loop {
                                    match node {
                                        Node::Internal(inode) => {
                                            internals.extend(
                                                inode.children()[..inode
                                                    .children()
                                                    .len()
                                                    - 1]
                                                    .iter()
                                                    .map(Arc::clone),
                                            );

                                            node = &**inode
                                                .children()
                                                .last()
                                                .unwrap();
                                        },

                                        Node::Leaf(leaf) => {
                                            break (
                                                leaf.value().borrow(),
                                                leaf.summary().clone(),
                                            );
                                        },
                                    }
                                }
                            };

                            return Some(TreeSlice {
                                span: SliceSpan::Multi {
                                    start,
                                    internals,
                                    end,
                                },
                                summary,
                            });
                        },

                        None => {
                            return Some(TreeSlice {
                                span: SliceSpan::Single(start.0),
                                summary,
                            });
                        },
                    }
                },
            }
        }

        let mut internals = Vec::new();

        let end = match next_bubugaga::<FANOUT, L, M>(
            self.root_nodes,
            &mut self.root_forward_idx,
            &mut self.forward_path,
            &mut self.start,
            &mut summary,
            &mut internals,
        ) {
            Some(end) => end,

            None => {
                debug_assert!(self.end.is_some(), "TODO: explain why");

                // Safety: TODO.
                let (end_slice, end_summary) =
                    unsafe { self.end.take().unwrap_unchecked() };

                // Same exact code as above
                if M::measure(&end_summary) == M::zero() {
                    (end_slice, end_summary)
                } else {
                    let (end_slice, end_summary, right) =
                        M::split_left(end_slice, M::one(), &end_summary);
                    self.end = right;
                    summary += &end_summary;
                    (end_slice, end_summary)
                }
            },
        };

        self.yielded_forward += M::one();

        Some(TreeSlice {
            span: SliceSpan::Multi { start, internals, end },
            summary,
        })
    }
}

fn next_something_forward<'a, const N: usize, L: Leaf>(
    root_nodes: &'a [Arc<Node<N, L>>],
    root_idx: &mut isize,
    path: &mut Vec<(&'a Inode<N, L>, usize)>,
) -> Option<(&'a L::Slice, &'a L::Summary)> {
    let mut inode = loop {
        match path.last_mut() {
            Some(&mut (inode, ref mut visited)) => {
                if inode.children().len() == *visited + 1 {
                    path.pop();
                } else {
                    *visited += 1;
                    match &*inode.children()[*visited] {
                        Node::Internal(inode) => {
                            break inode;
                        },
                        Node::Leaf(leaf) => {
                            return Some((
                                leaf.value().borrow(),
                                leaf.summary(),
                            ));
                        },
                    }
                }
            },

            None => {
                if root_nodes.len() == (*root_idx + 1) as usize {
                    return None;
                } else {
                    *root_idx += 1;
                    match &*root_nodes[*root_idx as usize] {
                        Node::Internal(inode) => {
                            break inode;
                        },

                        Node::Leaf(leaf) => {
                            return Some((
                                leaf.value().borrow(),
                                leaf.summary(),
                            ));
                        },
                    }
                }
            },
        }
    };

    loop {
        path.push((inode, 0));
        match &*inode.children()[0] {
            Node::Internal(i) => {
                inode = i;
            },
            Node::Leaf(leaf) => {
                return Some((leaf.value().borrow(), leaf.summary()));
            },
        }
    }
}

fn next_bubugaga<'a, const N: usize, L: Leaf, M: Metric<L>>(
    root_nodes: &'a [Arc<Node<N, L>>],
    root_idx: &mut isize,
    path: &mut Vec<(&'a Inode<N, L>, usize)>,
    next_start: &mut Option<(&'a L::Slice, L::Summary)>,
    summary: &mut L::Summary,
    internals: &mut Vec<Arc<Node<N, L>>>,
) -> Option<(&'a L::Slice, L::Summary)> {
    let mut inode = loop {
        match path.last_mut() {
            Some(&mut (inode, ref mut visited)) => {
                if inode.children().len() == *visited + 1 {
                    path.pop();
                } else {
                    *visited += 1;
                    match &*inode.children()[*visited] {
                        Node::Internal(i) => {
                            if M::measure(i.summary()) == M::zero() {
                                *summary += i.summary();
                                internals.push(Arc::clone(
                                    &inode.children()[*visited],
                                ));
                            } else {
                                break i;
                            }
                        },

                        Node::Leaf(leaf) => {
                            if M::measure(leaf.summary()) == M::zero() {
                                *summary += leaf.summary();
                                internals.push(Arc::clone(
                                    &inode.children()[*visited],
                                ));
                            } else {
                                let (end_slice, end_summary, right) =
                                    M::split_left(
                                        leaf.value().borrow(),
                                        M::one(),
                                        leaf.summary(),
                                    );
                                *summary += &end_summary;
                                *next_start = right;
                                return Some((end_slice, end_summary));
                            }
                        },
                    }
                }
            },

            None => {
                if root_nodes.len() == (*root_idx + 1) as usize {
                    return None;
                } else {
                    *root_idx += 1;
                    match &*root_nodes[*root_idx as usize] {
                        Node::Internal(inode) => {
                            if M::measure(inode.summary()) == M::zero() {
                                *summary += inode.summary();
                                internals.push(Arc::clone(
                                    &root_nodes[*root_idx as usize],
                                ));
                            } else {
                                break inode;
                            }
                        },

                        Node::Leaf(leaf) => {
                            if M::measure(leaf.summary()) == M::zero() {
                                *summary += leaf.summary();
                                internals.push(Arc::clone(
                                    &root_nodes[*root_idx as usize],
                                ));
                            } else {
                                let (end_slice, end_summary, right) =
                                    M::split_left(
                                        leaf.value().borrow(),
                                        M::one(),
                                        leaf.summary(),
                                    );
                                *summary += &end_summary;
                                *next_start = right;
                                return Some((end_slice, end_summary));
                            }
                        },
                    }
                }
            },
        }
    };

    'outer: loop {
        for (idx, child) in inode.children().iter().enumerate() {
            match &**child {
                Node::Internal(i) => {
                    if M::measure(i.summary()) == M::zero() {
                        *summary += i.summary();
                        internals.push(Arc::clone(child));
                    } else {
                        path.push((inode, idx));
                        inode = i;
                        continue 'outer;
                    }
                },

                Node::Leaf(leaf) => {
                    if M::measure(leaf.summary()) == M::zero() {
                        *summary += leaf.summary();
                        internals.push(Arc::clone(child));
                    } else {
                        path.push((inode, idx));
                        let (end_slice, end_summary, right) = M::split_left(
                            leaf.value().borrow(),
                            M::one(),
                            leaf.summary(),
                        );
                        *summary += &end_summary;
                        *next_start = right;
                        return Some((end_slice, end_summary));
                    }
                },
            }
        }
    }
}

fn next_babagugu<'a, const N: usize, L: Leaf, M: Metric<L>>(
    root_nodes: &'a [Arc<Node<N, L>>],
    root_idx: &mut isize,
    path: &mut Vec<(&'a Inode<N, L>, usize)>,
    summary: &mut L::Summary,
    internals: &mut Vec<&'a Arc<Node<N, L>>>,
) {
    loop {
        match path.last_mut() {
            Some(&mut (inode, ref mut visited)) => {
                if inode.children().len() == *visited + 1 {
                    path.pop();
                } else {
                    *visited += 1;
                    *summary += inode.children()[*visited].summary();
                    internals.push(&inode.children()[*visited]);
                }
            },

            None => {
                if root_nodes.len() == (*root_idx + 1) as usize {
                    return;
                } else {
                    *root_idx += 1;
                    *summary += root_nodes[*root_idx as usize].summary();
                    internals.push(&root_nodes[*root_idx as usize]);
                }
            },
        }
    }
}

impl<'a, const FANOUT: usize, L: Leaf, M: Metric<L>> DoubleEndedIterator
    for Units<'a, FANOUT, L, M>
{
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        todo!()
    }
}

impl<'a, const FANOUT: usize, L: Leaf, M: Metric<L>> std::iter::FusedIterator
    for Units<'a, FANOUT, L, M>
{
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leaves_1() {
        for n in 1..100 {
            let tree = Tree::<2, usize>::from_leaves(0..n);
            let mut leaves = tree.leaves();
            let mut i = 0;
            while let Some(leaf) = leaves.next() {
                assert_eq!(i, *leaf);
                i += 1;
            }
            assert_eq!(i, n);
            assert_eq!(None, leaves.next());
            assert_eq!(None, leaves.next());
        }
    }

    #[ignore]
    #[test]
    fn leaves_2() {
        let tree = Tree::<2, usize>::from_leaves(0..3);
        let mut leaves = tree.leaves();

        assert_eq!(Some(&2), leaves.next_back());
        assert_eq!(Some(&1), leaves.next_back());
        assert_eq!(Some(&0), leaves.next_back());
        assert_eq!(None, leaves.next_back());
        assert_eq!(None, leaves.next_back());
    }
}
