use point::Point;
use dcel::{DCEL, Vertex};
use beachline::*;
use event::*;
use geometry::*;

type TripleSite = (Point, Point, Point);

pub fn voronoi(points: Vec<Point>) -> DCEL {
	trace!("Starting Voronoi Computation");
	let mut event_queue = EventQueue::new();
	let mut beachline = BeachLine::new();
	for pt in points {
		event_queue.push(VoronoiEvent::Site { 0: pt }, &mut beachline);
	}
	let mut result = DCEL::new();

	while !event_queue.is_empty() {
		trace!("\n\n");
		trace!("Beachline: {}", beachline);
		trace!("Queue: {}", event_queue);
		let this_event = event_queue.pop(&mut beachline).unwrap();
		trace!("Popped event from queue: {}", this_event);
		handle_event(this_event, &mut event_queue, &mut beachline, &mut result);
	}
	add_bounding_box(&beachline, &mut result);
	return result;
}

fn handle_event(this_event: VoronoiEvent, queue: &mut EventQueue, beachline: &mut BeachLine, result: &mut DCEL) {
	match this_event {
		VoronoiEvent::Site(pt) => { handle_site_event(pt, queue, beachline, result); },
		VoronoiEvent::Circle(leaf, triplesite) => { handle_circle_event(leaf, triplesite, queue, beachline, result); }
	}
}

fn handle_site_event(site: Point, queue: &mut EventQueue, beachline: &mut BeachLine, result: &mut DCEL) {
	trace!("Handling site event at {}", site);
	if beachline.is_empty() {
		trace!("Beachline was empty, inserting point.");
		beachline.insert_point(site);
		return;
	}
	
	let arc_above = beachline.get_arc_above(site);

	// remove false alarm from queue
	remove_circle_event(arc_above, queue, beachline);

	let new_node = split_arc(arc_above, site, beachline, result);

	if let Some(left_triple) = beachline.get_leftward_triple(new_node) {
		trace!("Checking leftward triple {}, {}, {}", left_triple.0, left_triple.1, left_triple.2);
		if breakpoints_converge(left_triple) {
			trace!("Found converging triple");
			let left_arc = beachline.get_left_arc(Some(new_node)).unwrap();
			make_circle_event(left_arc, left_triple, queue, beachline);
		}
	}
	if let Some(right_triple) = beachline.get_rightward_triple(new_node) {
		trace!("Checking rightward triple {}, {}, {}", right_triple.0, right_triple.1, right_triple.2);
		if breakpoints_converge(right_triple) {
			trace!("Found converging triple");
			let right_arc = beachline.get_right_arc(Some(new_node)).unwrap();
			make_circle_event(right_arc, right_triple, queue, beachline);
		}
	}
}

fn remove_circle_event(this_arc: usize, queue: &mut EventQueue, beachline: &mut BeachLine) {
	let mut circle_event = None;
	if let BeachItem::Leaf(ref mut arc) = beachline.nodes[this_arc].item {
		circle_event = arc.site_event;
		arc.site_event = None;
	}
	if let Some(circle_node) = circle_event {
		queue.remove(circle_node, beachline);
	}
}

fn make_circle_event(leaf: usize, triple: TripleSite, queue: &mut EventQueue, beachline: &mut BeachLine) {
	let this_event = VoronoiEvent::Circle {0: leaf, 1: triple};
	let circle_event_ind = queue.events.len();
	if let BeachItem::Leaf(ref mut arc) = beachline.nodes[leaf].item {
		arc.site_event = Some(circle_event_ind);
	}
	queue.push(this_event, beachline);
}

#[allow(non_snake_case)]
// return: the index of the node for the new arc
fn split_arc(arc: usize, pt: Point, beachline: &mut BeachLine, dcel: &mut DCEL) -> usize {
	trace!("Splitting arc {}", arc);
	let parent = beachline.nodes[arc].parent;

	let mut arc_pt = Point::new(0.0, 0.0);
	if let BeachItem::Leaf(ref this_arc) = beachline.nodes[arc].item {
		arc_pt = this_arc.site;
	}

	let (twin1, twin2) = dcel.add_twins();
	
	let breakpoint_AB = BreakPoint::new(arc_pt, pt, twin1);
	let breakpoint_BA = BreakPoint::new(pt, arc_pt, twin2);

	let internal_AB = BeachItem::Internal(breakpoint_AB);
	let internal_BA = BeachItem::Internal(breakpoint_BA);

	let arc_A1 = Arc::new(arc_pt, None);
	let arc_A2 = Arc::new(arc_pt, None);
	let arc_B = Arc::new(pt, None);

	let leaf_A1 = BeachItem::Leaf(arc_A1);
	let leaf_A2 = BeachItem::Leaf(arc_A2);
	let leaf_B = BeachItem::Leaf(arc_B);

	let ind_AB = beachline.nodes.len();
	let ind_BA = ind_AB + 1;
	let ind_A1 = ind_AB + 2;
	let ind_B  = ind_AB + 3;
	let ind_A2 = ind_AB + 4;

	let node_AB = BeachNode { parent: parent, left_child: Some(ind_A1), right_child: Some(ind_BA), item: internal_AB};
	beachline.nodes.push(node_AB);
	if let Some(parent_ind) = parent {
		let parent_node = &mut beachline.nodes[parent_ind];
		if parent_node.right_child.is_some() && parent_node.right_child.unwrap() == arc {
			parent_node.right_child = Some(ind_AB);
		} else if parent_node.left_child.is_some() && parent_node.left_child.unwrap() == arc {
			parent_node.left_child = Some(ind_AB);
		} else {
			panic!("tree is borked");
		}
	} else {
		beachline.root = ind_AB;
	}

	let node_BA = BeachNode {parent: Some(ind_AB), left_child: Some(ind_B), right_child: Some(ind_A2), item: internal_BA};
	beachline.nodes.push(node_BA);

	let node_A1 = BeachNode::make_arc(Some(ind_AB), leaf_A1);
	beachline.nodes.push(node_A1);

	let node_B = BeachNode::make_arc(Some(ind_BA), leaf_B);
	beachline.nodes.push(node_B);

	let node_A2 = BeachNode::make_arc(Some(ind_BA), leaf_A2);
	beachline.nodes.push(node_A2);

	return ind_B;
}

// return: indices of predecessor, successor, parent, 'other'
// where 'other' is the one of predecessor or sucessor that
// is not the parent of the leaf.
fn delete_leaf(leaf: usize, beachline: &mut BeachLine) -> (usize, usize, usize, usize) {
	let pred = beachline.predecessor(leaf).unwrap();
	let succ = beachline.successor(leaf).unwrap();
	let parent = beachline.nodes[leaf].parent.unwrap();
	let grandparent = beachline.nodes[parent].parent.unwrap();
	
	let other = if parent == pred { succ } else { pred };

	// find sibling
	let sibling;
	if beachline.nodes[parent].right_child.unwrap() == leaf {
		sibling = beachline.nodes[parent].left_child.unwrap();
	} else if beachline.nodes[parent].left_child.unwrap() == leaf {
		sibling = beachline.nodes[parent].right_child.unwrap();
	} else {
		panic!("family strife! parent does not acknowledge leaf!");
	}

	// transplant the sibling to replace the parent
	beachline.nodes[sibling].parent = Some(grandparent);
	if beachline.nodes[grandparent].left_child.unwrap() == parent {
		beachline.nodes[grandparent].left_child = Some(sibling);
	} else if beachline.nodes[grandparent].right_child.unwrap() == parent {
		beachline.nodes[grandparent].right_child = Some(sibling);
	} else {
		panic!("family strife! grandparent does not acknowledge parent!");
	}

	// correct the site on 'other'
	if other == pred {
		let new_other_succ = beachline.successor(other).unwrap();
		let new_site = beachline.get_site(Some(new_other_succ)).unwrap();
		beachline.set_right_site(other, new_site);
	} else {
		let new_other_pred = beachline.predecessor(other).unwrap();
		let new_site = beachline.get_site(Some(new_other_pred)).unwrap();
		beachline.set_left_site(other, new_site);
	}

	(pred, succ, parent, other)
}

fn handle_circle_event(
	leaf: usize,
	triplesite: TripleSite,
	queue: &mut EventQueue,
	beachline: &mut BeachLine,
	dcel: &mut DCEL) {

	let left_neighbor = beachline.get_left_arc(Some(leaf)).unwrap();
	let right_neighbor = beachline.get_right_arc(Some(leaf)).unwrap();
	let (pred, succ, parent, other) = delete_leaf(leaf, beachline);

	// removing site events involving disappearing arc
	remove_circle_event(leaf, queue, beachline);
	remove_circle_event(left_neighbor, queue, beachline);
	remove_circle_event(right_neighbor, queue, beachline);

	let (twin1, twin2) = dcel.add_twins();

	// make a vertex at the circle center
	let circle_center = circle_center(triplesite);
	let center_vertex = Vertex { coordinates: circle_center, incident_edge: twin1};
	let center_vertex_ind = dcel.vertices.len();
	dcel.vertices.push(center_vertex);

	// hook up next pointers on halfedges
	let pred_edge = beachline.get_edge(pred);
	let succ_edge = beachline.get_edge(succ);
	let parent_edge = beachline.get_edge(parent);
	let other_edge = beachline.get_edge(other);

	let pred_edge_twin = dcel.halfedges[pred_edge].twin;
	let succ_edge_twin = dcel.halfedges[succ_edge].twin;

	dcel.halfedges[parent_edge].origin = center_vertex_ind;
	dcel.halfedges[other_edge].origin = center_vertex_ind;
	dcel.halfedges[twin1].origin = center_vertex_ind;

	dcel.halfedges[pred_edge_twin].next = succ_edge;
	dcel.halfedges[succ_edge_twin].next = twin1;
	dcel.halfedges[twin2].next = pred_edge;

	if let BeachItem::Internal(ref mut breakpoint) = beachline.nodes[other].item {
		breakpoint.halfedge = twin2;
	}

	if let Some(left_triple) = beachline.get_centered_triple(left_neighbor) {
		trace!("Checking leftward triple {}, {}, {}", left_triple.0, left_triple.1, left_triple.2);
		if breakpoints_converge(left_triple) {
			trace!("Found converging triple");
			make_circle_event(left_neighbor, left_triple, queue, beachline);
		}
	}
	if let Some(right_triple) = beachline.get_centered_triple(right_neighbor) {
		trace!("Checking rightward triple {}, {}, {}", right_triple.0, right_triple.1, right_triple.2);
		if breakpoints_converge(right_triple) {
			trace!("Found converging triple");
			make_circle_event(right_neighbor, right_triple, queue, beachline);
		}
	}
}

// This just extends the edges past the end of the bounding box
fn add_bounding_box(beachline: &BeachLine, dcel: &mut DCEL) {
	let mut current_node = beachline.tree_minimum(beachline.root);
	trace!("\n\n");
	loop {
		match beachline.nodes[current_node].item {
			BeachItem::Leaf(_) => {},
			BeachItem::Internal(ref breakpoint) => {
				let this_edge = breakpoint.halfedge;
				trace!("Extending halfedge {} with breakpoint {}, {}", this_edge, breakpoint.left_site, breakpoint.right_site);
				let this_x = get_breakpoint_x(&breakpoint, -1000.0);
				let this_y = get_breakpoint_y(&breakpoint, -1000.0);

				let vert = Vertex {coordinates: Point::new(this_x, this_y), incident_edge: this_edge};
				let vert_ind = dcel.vertices.len();

				dcel.halfedges[this_edge].origin = vert_ind;
				let this_twin = dcel.halfedges[this_edge].twin;
				dcel.halfedges[this_twin].next = this_edge;

				dcel.vertices.push(vert);
			}
		}
		if let Some(next_node) = beachline.successor(current_node) {
			current_node = next_node;
		} else { break; }
	}

}